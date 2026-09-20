# User Guide

Sermon Studio is a writing studio first and a retrieval engine second. This
guide covers the daily workflow.

## The vault

Your sermons live in a single folder — the **vault** — chosen on first run
(default `~/Sermons`). Each sermon is one `.md` file:

```markdown
---
id: the-grace-that-saves
title: "The Grace That Saves"
date_preached: 2024-03-10
series: "Ephesians: Riches in Christ"
liturgical_season: Lent
primary_passage: "Eph.2.8-Eph.2.9"
exegetical_proposition: "Salvation is by grace through faith, not works."
big_idea: "Grace is God's free gift; faith is the empty hand that receives it."
structure_type: verse_by_verse
illustrations:
  - "The drowning man and the lifeguard"
---

# The Grace That Saves

## Text

> Ephesians 2:8-9

## Exegetical Proposition
...
```

Because it is plain text you can `grep`, `git`, `rsync`, or open it in `vim`.
Sermon Studio never rewrites your prose — it round-trips CommonMark exactly.

## Writing

* **New sermon** — `Ctrl+N`. You are prompted for a title and a primary passage;
  a structured template is created and opened.
* **Save** — `Ctrl+S`. The file is written and the index is updated in the same
  action. There is no separate "index" step.
* **Command palette** — `Ctrl+P`. Every action is reachable here.
* **Search** — `Ctrl+K`, then type. Results update as you type and show a
  highlighted snippet.

There are no modal dialogs while you write. A transient toast confirms saves.

## The study panel

When a sermon is open, the right-hand panel shows:

* **Passage** — the KJV text of the primary passage.
* **Interlinear (Strong's)** — each word of the first verse with its Strong's
  number. Click a number to see the lexicon entry (lemma, transliteration,
  gloss, definition, derivation).
* **Cross References** — the top TSK/OpenBible cross-references for the first
  verse, ranked. Click one to search the archive for it.
* **Preached On This Text** — other sermons in your archive that touch the same
  passage. Click to open.
* **Related (Librarian)** — only when the Librarian is enabled.

## Illustration fatigue

Reusing the same illustration across many sermons is easy to do and hard to
notice. The fatigue report lists illustrations used more than once, with a count
and the number of sermons involved. Run it from the command palette
("Show Illustration Fatigue") or:

```bash
sermon fatigue --db ~/.local/share/sermon-studio/pastor.db --min 3
```

## The Librarian

The Librarian is an **optional, local, off-by-default** cataloger. When enabled
it computes bag-of-words cosine similarity between sermons to surface related
sermons and reused illustrations. It uses no model weights and no network. Toggle
it from the command palette or by clicking the "Librarian" chip in the top bar.

## Rebuilding the index

`pastor.db` is a cache. If it is ever corrupted or deleted, rebuild it:

```bash
sermon index --vault ~/Sermons --db ~/.local/share/sermon-studio/pastor.db
```

Or press "Rebuild Index" in the top bar. A 200-sermon archive rebuilds in about
a third of a second. **No sermon content is ever stored only in the database.**

## Keyboard reference

| Shortcut | Action |
| --- | --- |
| `Ctrl+P` | Command palette |
| `Ctrl+S` | Save |
| `Ctrl+N` | New sermon |
| `Ctrl+K` | Search |
| `Ctrl+1` | Archive panel |
| `Ctrl+2` | Editor |
| `Ctrl+3` | Study panel |
| `Ctrl+B` | Bold |
| `Ctrl+I` | Italic |
| `Esc` | Dismiss / blur |

## Accessibility

The default theme is high-contrast dark with a 17px editor font. Both are
configurable in `~/.config/sermon-studio/config.json` (`high_contrast`,
`font_size`). The interface is fully keyboard-navigable.
