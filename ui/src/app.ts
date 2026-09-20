// Sermon Studio — frontend application.
//
// Design principles (from the user persona):
//   * Zero modal popups during writing. Feedback is a transient toast; commands
//     live in a keyboard-driven palette overlay.
//   * Full keyboard navigation. Ctrl+P palette, Ctrl+S save, Ctrl+N new,
//     Ctrl+K search, Ctrl+1/2/3 focus panels, Ctrl+B/I editor marks.
//   * High-contrast, low-friction, plain-text-first.
//   * The Librarian (AI) is invisible and OFF by default.

import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import { Markdown } from "tiptap-markdown";
import { api, type SermonHit, type VerseRow } from "./api";
import "./styles.css";

// ---------------------------------------------------------------------------
// Frontmatter (minimal YAML subset: scalars + string lists)
// ---------------------------------------------------------------------------
interface Frontmatter {
  id?: string;
  title?: string;
  date_preached?: string;
  series?: string;
  liturgical_season?: string;
  primary_passage?: string;
  exegetical_proposition?: string;
  big_idea?: string;
  structure_type?: string;
  illustrations?: string[];
  [k: string]: unknown;
}

function parseDoc(raw: string): { fm: Frontmatter; body: string } {
  const text = raw.replace(/^\uFEFF/, "");
  if (!text.startsWith("---")) return { fm: {}, body: text };
  const end = text.indexOf("\n---", 3);
  if (end === -1) return { fm: {}, body: text };
  const fmText = text.slice(3, end).trim();
  const body = text.slice(end + 4).replace(/^\r?\n/, "");
  const fm: Frontmatter = {};
  let currentList: string | null = null;
  for (const line of fmText.split("\n")) {
    const listMatch = line.match(/^\s*-\s+(.*)$/);
    if (listMatch && currentList) {
      (fm[currentList] as string[]).push(unquote(listMatch[1]));
      continue;
    }
    const kv = line.match(/^([A-Za-z0-9_]+):\s*(.*)$/);
    if (!kv) continue;
    const key = kv[1];
    const val = kv[2].trim();
    if (val === "" || val === "[]") {
      fm[key] = [];
      currentList = key;
    } else if (val.startsWith("[") && val.endsWith("]")) {
      fm[key] = val
        .slice(1, -1)
        .split(",")
        .map((s) => unquote(s.trim()))
        .filter(Boolean);
      currentList = null;
    } else {
      fm[key] = unquote(val);
      currentList = null;
    }
  }
  return { fm, body };
}

function unquote(s: string): string {
  s = s.trim();
  if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) {
    return s.slice(1, -1);
  }
  return s;
}

function serializeDoc(fm: Frontmatter, body: string): string {
  const order = [
    "id", "title", "date_preached", "series", "liturgical_season",
    "primary_passage", "exegetical_proposition", "big_idea", "structure_type",
  ];
  const lines: string[] = ["---"];
  for (const key of order) {
    const v = fm[key];
    if (v === undefined || v === null || v === "") continue;
    lines.push(`${key}: ${quote(String(v))}`);
  }
  const illus = (fm.illustrations as string[]) || [];
  if (illus.length) {
    lines.push("illustrations:");
    for (const i of illus) lines.push(`  - ${quote(i)}`);
  } else {
    lines.push("illustrations: []");
  }
  lines.push("---", "");
  return lines.join("\n") + body;
}

function quote(s: string): string {
  if (s === "") return '""';
  if (/[:#\[\]{}",]/.test(s) || s !== s.trim()) return `"${s.replace(/"/g, '\\"')}"`;
  return s;
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------
interface State {
  config: Awaited<ReturnType<typeof api.getConfig>> | null;
  sermons: SermonHit[];
  activeFile: string | null;
  activeId: string | null;
  dirty: boolean;
  fm: Frontmatter;
  editor: Editor | null;
  paletteIndex: number;
  paletteItems: { label: string; hint: string; run: () => void }[];
}

const state: State = {
  config: null,
  sermons: [],
  activeFile: null,
  activeId: null,
  dirty: false,
  fm: {},
  editor: null,
  paletteIndex: 0,
  paletteItems: [],
};

// ---------------------------------------------------------------------------
// DOM helpers
// ---------------------------------------------------------------------------
const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const el = (tag: string, cls?: string, text?: string) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
};

function toast(msg: string) {
  const t = $("#toast");
  t.textContent = msg;
  t.classList.add("show");
  window.clearTimeout((t as any)._timer);
  (t as any)._timer = window.setTimeout(() => t.classList.remove("show"), 1800);
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------
function buildLayout() {
  const app = $("#app");
  app.innerHTML = `
    <div class="topbar">
      <span class="brand">Sermon Studio</span>
      <button id="btn-new" title="New sermon (Ctrl+N)">New</button>
      <button id="btn-save" title="Save (Ctrl+S)">Save</button>
      <button id="btn-rebuild" title="Rebuild index from disk">Rebuild Index</button>
      <span class="spacer"></span>
      <span class="chip" id="chip-librarian" title="Optional offline cataloger (off by default)">Librarian: off</span>
      <span class="chip" id="chip-vault" title="Sermon vault"></span>
    </div>

    <aside class="panel left">
      <div class="search-wrap">
        <input id="search" type="search" placeholder="Search sermons…  (Ctrl+K)" autocomplete="off" />
      </div>
      <h2>Archive</h2>
      <ul class="sermon-list" id="sermon-list"></ul>
    </aside>

    <main class="center">
      <div class="meta-bar">
        <div class="field wide"><label>Title</label><input id="f-title" /></div>
        <div class="field"><label>Date</label><input id="f-date" placeholder="YYYY-MM-DD" /></div>
        <div class="field"><label>Series</label><input id="f-series" /></div>
        <div class="field"><label>Season</label><input id="f-season" /></div>
        <div class="field"><label>Primary Passage</label><input id="f-passage" placeholder="Rom 8:28-30" /></div>
        <div class="field"><label>Structure</label>
          <select id="f-structure">
            <option value="verse_by_verse">verse_by_verse</option>
            <option value="narrative">narrative</option>
            <option value="deductive">deductive</option>
          </select>
        </div>
        <div class="field wide"><label>Big Idea</label><input id="f-bigidea" /></div>
        <div class="field wide"><label>Exegetical Proposition</label><input id="f-exeg" /></div>
      </div>
      <div class="editor-scroll"><div class="editor" id="editor"></div></div>
    </main>

    <aside class="panel right">
      <h2>Study</h2>
      <div class="study-section" id="study">
        <div class="empty">Open a sermon to see its passage, lexicon, and cross-references.</div>
      </div>
    </aside>

    <div class="statusbar">
      <span id="st-vault">vault: —</span>
      <span id="st-count">0 sermons</span>
      <span class="spacer"></span>
      <span id="st-msg" class="ok">ready</span>
      <span id="st-saved">saved</span>
    </div>

    <div id="toast"></div>
    <div id="palette">
      <input id="palette-input" placeholder="Type a command…" autocomplete="off" />
      <ul id="palette-list"></ul>
    </div>
  `;
}

// ---------------------------------------------------------------------------
// Editor
// ---------------------------------------------------------------------------
function initEditor() {
  state.editor = new Editor({
    element: $("#editor"),
    extensions: [
      StarterKit.configure({ heading: { levels: [1, 2, 3] } }),
      Placeholder.configure({ placeholder: "Write the sermon…" }),
      Markdown.configure({ html: false, tightLists: true, linkify: false }),
    ],
    content: "",
    onUpdate: () => {
      state.dirty = true;
      updateStatus();
    },
  });
}

function setEditorContent(markdown: string) {
  state.editor?.commands.setContent(markdown, false);
}

function getEditorMarkdown(): string {
  const storage = state.editor?.storage as any;
  if (storage?.markdown?.getMarkdown) return storage.markdown.getMarkdown();
  return state.editor?.getText() ?? "";
}

// ---------------------------------------------------------------------------
// Meta bar binding
// ---------------------------------------------------------------------------
function bindMeta() {
  const map: [string, string][] = [
    ["#f-title", "title"],
    ["#f-date", "date_preached"],
    ["#f-series", "series"],
    ["#f-season", "liturgical_season"],
    ["#f-passage", "primary_passage"],
    ["#f-structure", "structure_type"],
    ["#f-bigidea", "big_idea"],
    ["#f-exeg", "exegetical_proposition"],
  ];
  for (const [sel, key] of map) {
    const input = $(sel) as HTMLInputElement;
    input.addEventListener("input", () => {
      state.fm[key] = input.value;
      state.dirty = true;
      updateStatus();
    });
  }
}

function fillMeta() {
  const set = (sel: string, v: unknown) => {
    const input = $(sel) as HTMLInputElement;
    input.value = v === undefined || v === null ? "" : String(v);
  };
  set("#f-title", state.fm.title);
  set("#f-date", state.fm.date_preached);
  set("#f-series", state.fm.series);
  set("#f-season", state.fm.liturgical_season);
  set("#f-passage", state.fm.primary_passage);
  set("#f-structure", state.fm.structure_type || "verse_by_verse");
  set("#f-bigidea", state.fm.big_idea);
  set("#f-exeg", state.fm.exegetical_proposition);
}

// ---------------------------------------------------------------------------
// Sermon list & search
// ---------------------------------------------------------------------------
function renderSermonList(hits: SermonHit[]) {
  const ul = $("#sermon-list");
  ul.innerHTML = "";
  if (!hits.length) {
    ul.appendChild(el("li", "empty", "No sermons yet. Press Ctrl+N to begin."));
    return;
  }
  for (const h of hits) {
    const li = el("li");
    if (h.file_path === state.activeFile) li.classList.add("active");
    li.appendChild(el("div", "t", h.title));
    const meta = el("div", "m");
    meta.appendChild(el("span", "p", h.primary_passage || "—"));
    if (h.date_preached) meta.appendChild(document.createTextNode("  ·  " + h.date_preached));
    if (h.series) meta.appendChild(document.createTextNode("  ·  " + h.series));
    li.appendChild(meta);
    if (h.snippet) {
      const snip = el("div", "snip");
      snip.innerHTML = escapeHtml(h.snippet).replace(/&lt;&lt;/g, "<mark>").replace(/&gt;&gt;/g, "</mark>");
      li.appendChild(snip);
    }
    li.addEventListener("click", () => openSermon(h));
    ul.appendChild(li);
  }
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

async function refreshList() {
  state.sermons = await api.listSermons();
  renderSermonList(state.sermons);
  $("#st-count").textContent = `${state.sermons.length} sermons`;
}

async function doSearch(q: string) {
  if (!q.trim()) {
    renderSermonList(state.sermons);
    return;
  }
  const hits = await api.searchSermons(q, 50);
  renderSermonList(hits);
}

// ---------------------------------------------------------------------------
// Open / save
// ---------------------------------------------------------------------------
async function openSermon(hit: SermonHit) {
  if (state.dirty && !(await confirmDiscard())) return;
  const raw = await api.readSermon(hit.file_path);
  const { fm, body } = parseDoc(raw);
  state.fm = fm;
  state.activeFile = hit.file_path;
  state.activeId = hit.id;
  state.dirty = false;
  fillMeta();
  setEditorContent(body);
  renderSermonList(state.sermons);
  updateStatus();
  await loadStudy(hit);
}

async function confirmDiscard(): Promise<boolean> {
  // Non-modal: use the native confirm only as a last resort; prefer toast+auto-save.
  // We auto-save instead of blocking the writer.
  await saveSermon(true);
  return true;
}

async function saveSermon(silent = false) {
  if (!state.activeFile) {
    if (!silent) toast("No sermon open");
    return;
  }
  const body = getEditorMarkdown();
  const content = serializeDoc(state.fm, body);
  try {
    await api.saveSermon(state.activeFile, content);
    state.dirty = false;
    updateStatus();
    if (!silent) toast("Saved");
    await refreshList();
  } catch (e) {
    toast("Save failed: " + e);
  }
}

async function newSermon() {
  const title = window.prompt("Sermon title?");
  if (!title) return;
  const passage = window.prompt("Primary passage (e.g. Rom 8:28-30)?") || "";
  const file = await api.newSermon(title, passage);
  await refreshList();
  const hit = state.sermons.find((s) => s.file_path === file);
  if (hit) await openSermon(hit);
  else {
    const raw = await api.readSermon(file);
    const { fm, body } = parseDoc(raw);
    state.fm = fm;
    state.activeFile = file;
    state.activeId = (fm.id as string) || null;
    fillMeta();
    setEditorContent(body);
  }
  toast("Created " + file);
}

// ---------------------------------------------------------------------------
// Study panel (passage, lexicon, cross-refs, preached, fatigue)
// ---------------------------------------------------------------------------
async function loadStudy(hit: SermonHit) {
  const study = $("#study");
  study.innerHTML = "";
  const passage = hit.primary_passage;
  if (!passage) {
    study.appendChild(el("div", "empty", "No primary passage set."));
    return;
  }

  // Passage text.
  let verses: VerseRow[] = [];
  try {
    verses = await api.getPassage(passage);
  } catch {
    verses = [];
  }
  const pSection = el("div");
  pSection.appendChild(el("h2", undefined, "Passage"));
  if (!verses.length) {
    pSection.appendChild(el("div", "empty", "Passage not found in canon."));
  }
  for (const v of verses) {
    const d = el("div", "verse");
    d.appendChild(el("span", "ref", `${v.book_name} ${v.chapter}:${v.verse}`));
    d.appendChild(document.createTextNode(v.text_kjv));
    pSection.appendChild(d);
  }
  study.appendChild(pSection);

  // Strong's words for the first verse.
  if (verses.length) {
    const first = verses[0];
    const words = await api.getVerseWords(first.book_num, first.chapter, first.verse);
    if (words.length) {
      const wSection = el("div");
      wSection.appendChild(el("h2", undefined, "Interlinear (Strong's)"));
      const wrap = el("div", "words");
      for (const w of words) {
        const chip = el("span", "word");
        chip.appendChild(document.createTextNode(w.surface_word));
        if (w.strong_id) {
          const s = el("span", "s", w.strong_id);
          chip.appendChild(s);
          chip.addEventListener("click", () => showLexicon(w.strong_id!));
        }
        wrap.appendChild(chip);
      }
      wSection.appendChild(wrap);
      wSection.appendChild(el("div", "lex", ""));
      study.appendChild(wSection);
    }

    // Cross references.
    const xrefs = await api.crossReferences(first.book_num, first.chapter, first.verse, 20);
    if (xrefs.length) {
      const xSection = el("div");
      xSection.appendChild(el("h2", undefined, "Cross References"));
      for (const [v, rank] of xrefs) {
        const d = el("div", "xref");
        d.appendChild(el("span", "rank", String(rank)));
        d.appendChild(document.createTextNode(`${v.book_name} ${v.chapter}:${v.verse}`));
        d.addEventListener("click", () => {
          ($("#search") as HTMLInputElement).value = `${v.book_name} ${v.chapter}:${v.verse}`;
          doSearch(`${v.book_name} ${v.chapter}:${v.verse}`);
        });
        xSection.appendChild(d);
      }
      study.appendChild(xSection);
    }
  }

  // Previously preached on this verse.
  try {
    const preached = await api.sermonsForVerse(passage);
    if (preached.length) {
      const sSection = el("div");
      sSection.appendChild(el("h2", undefined, "Preached On This Text"));
      for (const p of preached) {
        const d = el("div", "hit");
        d.appendChild(document.createTextNode(p.title + "  "));
        d.appendChild(el("span", "d", p.date_preached || ""));
        d.addEventListener("click", () => openSermon(p));
        sSection.appendChild(d);
      }
      study.appendChild(sSection);
    }
  } catch {
    /* ignore */
  }

  // Librarian (only if enabled).
  if (state.config?.librarian_enabled && state.activeId) {
    const related = await api.librarianRelated(state.activeId, 5);
    if (related.length) {
      const rSection = el("div");
      rSection.appendChild(el("h2", undefined, "Related (Librarian)"));
      for (const r of related) {
        const d = el("div", "hit");
        d.appendChild(document.createTextNode(r.title + "  "));
        d.appendChild(el("span", "d", `${r.shared_verses} shared verses`));
        rSection.appendChild(d);
      }
      study.appendChild(rSection);
    }
  }
}

async function showLexicon(strongId: string) {
  const lex = await api.getLexicon(strongId);
  const box = $(".lex");
  if (!box) return;
  box.innerHTML = "";
  if (!lex) {
    box.textContent = "No lexicon entry for " + strongId;
    return;
  }
  const head = el("div");
  head.appendChild(el("span", "lemma", lex.lemma));
  head.appendChild(el("span", "translit", lex.transliteration));
  box.appendChild(head);
  box.appendChild(el("div", "gloss", lex.gloss));
  box.appendChild(el("div", "def", lex.definition));
  if (lex.derivation) box.appendChild(el("div", "deriv", lex.derivation));
  box.appendChild(el("div", "deriv", `${lex.strong_id} · ${lex.testament}`));
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------
function updateStatus() {
  $("#st-saved").textContent = state.dirty ? "● unsaved" : "saved";
  ($("#st-saved") as HTMLElement).className = state.dirty ? "warn" : "ok";
}

function setStatus(msg: string, cls = "ok") {
  const s = $("#st-msg");
  s.textContent = msg;
  s.className = cls;
}

// ---------------------------------------------------------------------------
// Command palette (keyboard, non-modal)
// ---------------------------------------------------------------------------
function paletteCommands(): { label: string; hint: string; run: () => void }[] {
  return [
    { label: "New Sermon", hint: "Ctrl+N", run: newSermon },
    { label: "Save", hint: "Ctrl+S", run: () => saveSermon() },
    { label: "Rebuild Index from Disk", hint: "", run: rebuildIndex },
    { label: "Sync Index (incremental)", hint: "", run: syncIndex },
    { label: "Focus Search", hint: "Ctrl+K", run: () => ($("#search") as HTMLInputElement).focus() },
    { label: "Toggle Librarian (offline cataloger)", hint: "", run: toggleLibrarian },
    { label: "Show Illustration Fatigue", hint: "", run: showFatigue },
    { label: "Archive Statistics", hint: "", run: showStats },
  ];
}

function openPalette() {
  state.paletteItems = paletteCommands();
  state.paletteIndex = 0;
  $("#palette").classList.add("open");
  const input = $("#palette-input") as HTMLInputElement;
  input.value = "";
  renderPalette("");
  input.focus();
}

function closePalette() {
  $("#palette").classList.remove("open");
}

function renderPalette(filter: string) {
  const list = $("#palette-list");
  list.innerHTML = "";
  const items = state.paletteItems.filter((i) =>
    i.label.toLowerCase().includes(filter.toLowerCase())
  );
  state.paletteItems = items;
  if (state.paletteIndex >= items.length) state.paletteIndex = 0;
  items.forEach((item, i) => {
    const li = el("li");
    if (i === state.paletteIndex) li.classList.add("sel");
    li.appendChild(el("span", undefined, item.label));
    li.appendChild(el("span", "k", item.hint));
    li.addEventListener("click", () => {
      closePalette();
      item.run();
    });
    list.appendChild(li);
  });
}

function paletteKey(e: KeyboardEvent) {
  const items = state.paletteItems;
  if (e.key === "ArrowDown") {
    state.paletteIndex = Math.min(state.paletteIndex + 1, items.length - 1);
    renderPalette(($("#palette-input") as HTMLInputElement).value);
    e.preventDefault();
  } else if (e.key === "ArrowUp") {
    state.paletteIndex = Math.max(state.paletteIndex - 1, 0);
    renderPalette(($("#palette-input") as HTMLInputElement).value);
    e.preventDefault();
  } else if (e.key === "Enter") {
    const item = items[state.paletteIndex];
    closePalette();
    item?.run();
    e.preventDefault();
  } else if (e.key === "Escape") {
    closePalette();
    e.preventDefault();
  }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------
async function rebuildIndex() {
  setStatus("rebuilding…", "warn");
  const s = await api.rebuildIndex();
  setStatus(`rebuilt ${s.indexed} sermons in ${s.elapsed_ms}ms`, "ok");
  toast(`Index rebuilt: ${s.indexed} sermons in ${s.elapsed_ms}ms`);
  await refreshList();
}

async function syncIndex() {
  const s = await api.syncIndex();
  setStatus(`synced: ${s.indexed} updated, ${s.skipped_unchanged} unchanged`, "ok");
  toast(`Synced: ${s.indexed} updated`);
  await refreshList();
}

async function toggleLibrarian() {
  if (!state.config) return;
  state.config.librarian_enabled = !state.config.librarian_enabled;
  await api.setConfig(state.config);
  updateLibrarianChip();
  toast("Librarian " + (state.config.librarian_enabled ? "enabled (offline)" : "disabled"));
}

function updateLibrarianChip() {
  const chip = $("#chip-librarian");
  const on = state.config?.librarian_enabled;
  chip.textContent = "Librarian: " + (on ? "on" : "off");
  chip.className = "chip" + (on ? " on" : "");
}

async function showFatigue() {
  const f = await api.illustrationFatigue(2);
  const study = $("#study");
  study.innerHTML = "";
  study.appendChild(el("h2", undefined, "Illustration Fatigue"));
  if (!f.length) {
    study.appendChild(el("div", "empty", "No repeated illustrations."));
    return;
  }
  for (const i of f) {
    const d = el("div", "fatigue");
    d.appendChild(el("span", "n", `${i.total_uses}× `));
    d.appendChild(document.createTextNode(i.label));
    d.appendChild(el("span", "d", `  (${i.sermon_count} sermons)`));
    study.appendChild(d);
  }
}

async function showStats() {
  const s = await api.archiveStats();
  const study = $("#study");
  study.innerHTML = "";
  study.appendChild(el("h2", undefined, "Archive Statistics"));
  const rows: [string, string][] = [
    ["Sermons", String(s.sermon_count)],
    ["Series", String(s.series_count)],
    ["Verse links", String(s.verse_links)],
    ["Illustrations", String(s.illustration_count)],
    ["Last rebuild", s.last_rebuild || "—"],
  ];
  for (const [k, v] of rows) {
    const d = el("div", "hit");
    d.appendChild(document.createTextNode(k + ": "));
    d.appendChild(el("span", "d", v));
    study.appendChild(d);
  }
}

// ---------------------------------------------------------------------------
// Keyboard
// ---------------------------------------------------------------------------
function bindKeys() {
  window.addEventListener("keydown", (e) => {
    const mod = e.ctrlKey || e.metaKey;
    if ($("#palette").classList.contains("open")) {
      paletteKey(e);
      return;
    }
    if (mod && e.key.toLowerCase() === "p") {
      openPalette();
      e.preventDefault();
    } else if (mod && e.key.toLowerCase() === "s") {
      saveSermon();
      e.preventDefault();
    } else if (mod && e.key.toLowerCase() === "n") {
      newSermon();
      e.preventDefault();
    } else if (mod && e.key.toLowerCase() === "k") {
      ($("#search") as HTMLInputElement).focus();
      e.preventDefault();
    } else if (mod && e.key === "1") {
      ($("#search") as HTMLInputElement).focus();
      e.preventDefault();
    } else if (mod && e.key === "2") {
      state.editor?.commands.focus();
      e.preventDefault();
    } else if (mod && e.key === "3") {
      $("#study").scrollIntoView();
      e.preventDefault();
    } else if (e.key === "Escape") {
      ($("#search") as HTMLInputElement).blur();
    }
  });
}

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------
async function boot() {
  buildLayout();
  initEditor();
  bindMeta();
  bindKeys();

  state.config = await api.getConfig();
  $("#chip-vault").textContent = state.config.vault_path;
  $("#st-vault").textContent = "vault: " + state.config.vault_path;
  updateLibrarianChip();
  if (!state.config.canon_present) {
    setStatus("canon.db missing — run `sermon build-canon`", "warn");
  }

  $("#btn-new").addEventListener("click", newSermon);
  $("#btn-save").addEventListener("click", () => saveSermon());
  $("#btn-rebuild").addEventListener("click", rebuildIndex);
  $("#chip-librarian").addEventListener("click", toggleLibrarian);

  const search = $("#search") as HTMLInputElement;
  let searchTimer = 0;
  search.addEventListener("input", () => {
    window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => doSearch(search.value), 120);
  });

  await refreshList();
  setStatus("ready", "ok");
}

boot().catch((e) => {
  document.body.innerHTML = `<pre style="color:#ff6b6b;padding:20px">Sermon Studio failed to start:\n${e}</pre>`;
});
