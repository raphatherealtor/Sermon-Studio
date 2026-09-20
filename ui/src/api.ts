// Typed wrappers around the Tauri IPC command layer.
import { invoke } from "@tauri-apps/api/core";

export interface ConfigView {
  vault_path: string;
  canon_path: string;
  pastor_path: string;
  librarian_enabled: boolean;
  font_size: number;
  high_contrast: boolean;
  canon_present: boolean;
}

export interface SermonHit {
  id: string;
  title: string;
  primary_passage: string;
  big_idea: string;
  date_preached: string | null;
  series: string | null;
  liturgical_season: string | null;
  structure_type: string;
  file_path: string;
  snippet: string;
  score: number;
}

export interface VerseRow {
  book_num: number;
  book_name: string;
  chapter: number;
  verse: number;
  text_kjv: string;
}

export interface WordRow {
  word_order: number;
  surface_word: string;
  strong_id: string | null;
  morphology: string | null;
}

export interface LexiconEntry {
  strong_id: string;
  testament: string;
  lemma: string;
  transliteration: string;
  pronunciation: string | null;
  part_of_speech: string | null;
  definition: string;
  gloss: string;
  derivation: string | null;
}

export interface IllustrationFatigue {
  illustration_key: string;
  label: string;
  total_uses: number;
  sermon_count: number;
  last_used: string | null;
}

export interface ArchiveStats {
  sermon_count: number;
  series_count: number;
  verse_links: number;
  illustration_count: number;
  last_rebuild: string | null;
}

export interface IndexStats {
  scanned: number;
  indexed: number;
  skipped_unchanged: number;
  removed: number;
  links: number;
  illustrations: number;
  elapsed_ms: number;
}

export interface RelatedSermon {
  id: string;
  title: string;
  primary_passage: string;
  similarity: number;
  shared_verses: number;
  shared_terms: number;
}

export interface CatalogSuggestion {
  sermon_id: string;
  suggested_tags: string[];
  related: RelatedSermon[];
  reused_illustrations: string[];
}

export const api = {
  getConfig: () => invoke<ConfigView>("get_config"),
  setVault: (vault_path: string) => invoke<ConfigView>("set_vault", { vaultPath: vault_path }),
  setConfig: (config: ConfigView) => invoke<void>("set_config", { config }),
  listSermons: () => invoke<SermonHit[]>("list_sermons"),
  searchSermons: (query: string, limit = 50) =>
    invoke<SermonHit[]>("search_sermons", { query, limit }),
  readSermon: (file_path: string) => invoke<string>("read_sermon", { filePath: file_path }),
  saveSermon: (file_path: string, content: string) =>
    invoke<void>("save_sermon", { filePath: file_path, content }),
  newSermon: (title: string, passage: string) =>
    invoke<string>("new_sermon", { title, passage }),
  rebuildIndex: () => invoke<IndexStats>("rebuild_index"),
  syncIndex: () => invoke<IndexStats>("sync_index"),
  archiveStats: () => invoke<ArchiveStats>("archive_stats"),
  getPassage: (reference: string) => invoke<VerseRow[]>("get_passage", { reference }),
  getVerseWords: (book_num: number, chapter: number, verse: number) =>
    invoke<WordRow[]>("get_verse_words", { bookNum: book_num, chapter, verse }),
  getLexicon: (strong_id: string) => invoke<LexiconEntry | null>("get_lexicon", { strongId: strong_id }),
  versesForStrong: (strong_id: string, limit = 100) =>
    invoke<VerseRow[]>("verses_for_strong", { strongId: strong_id, limit }),
  crossReferences: (book_num: number, chapter: number, verse: number, limit = 25) =>
    invoke<[VerseRow, number][]>("cross_references", { bookNum: book_num, chapter, verse, limit }),
  sermonsForVerse: (reference: string) =>
    invoke<SermonHit[]>("sermons_for_verse", { reference }),
  illustrationFatigue: (min_uses = 2) =>
    invoke<IllustrationFatigue[]>("illustration_fatigue", { minUses: min_uses }),
  librarianCatalog: (sermon_id: string) =>
    invoke<CatalogSuggestion | null>("librarian_catalog", { sermonId: sermon_id }),
  librarianRelated: (sermon_id: string, limit = 5) =>
    invoke<RelatedSermon[]>("librarian_related", { sermonId: sermon_id, limit }),
};
