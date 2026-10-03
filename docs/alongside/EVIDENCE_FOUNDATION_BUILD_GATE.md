# Alongside evidence foundation — Build Gate 0

Branch: `feature/alongside-evidence-foundation`
Base: `integration/release-v1-final` (not the older `main` frontend, and NOT Rocket `raphatherealtor/sermonstudio` PR #4).
Status: foundation contract only; no runtime wiring or merge performed.

## Objective
Give Rafael a quiet, inspectable companion to his original sermon archive. The system recognizes useful recurrence without authoring, interpreting theology on his behalf, or silently touching a manuscript. "The echo never speaks before the voice."

## Canonical boundaries
1. Never replace the original PDF, filename, wording, punctuation, image or checksum. Print title, filename and credits are different metadata fields. Preserve page furniture separately from body.
2. Analysis must cite artifact + immutable revision hash + location + exact retained span. Missing coordinates remain missing.
3. Provenance status (visible-in-artifact, externally-verified, candidate, unresolved) is independent of the finding status (direct, structurally-derived, candidate, unresolved).
4. Source identity and source USE are separate. Borrowed first-person voice is not automatically Rafael's biography. A printed "Edited by" does not prove which spans were edited.
5. LNMI unit grid stays canonical and unchanged: VEC_COMP requires two opposed active headings in one traversal; RATE only F/S/K/I/unmarked; WEIGHT only H/L/C/unmarked. Metaphor, typology, list, punctuation, source citation, and running header DO NOT auto-trigger these labels.
6. Preserve meta-patterns (anchor/render/develop/distinguish/reposition/handoff; anchor/relate/traverse/reposition/handoff) as OPTIONAL arrangement observations backed by spans, NOT a universal template. Multiple functional readings may coexist.
7. "At least four" followed by five is not a logical contradiction. Never auto-mark it as such. Do not derive 16/16 claims from one witness's synthesis without per-record audit.
8. Do not treat the sixteen ZCode files as sixteen independent witnesses. Qwen and Grok are separate corpus-level witnesses; Vibe and ZCode each have sixteen records.
9. No canonical sermon Markdown mutation before explicit user APPLY. ASK / SUGGEST / PREVIEW / APPLY are distinct. Analysis is ephemeral or stored as separately versioned sidecar.
10. Desktop/offline-first: Rust owns validated local bytes, hashing, persistence, source alignment and index; TypeScript contract and UI mirror, never a competing canonical truth.

## Build Tracks (freeze this contract before parallel edits)

A. Intake & Evidence (Rust owner): attach sixteen scans as opaque immutable artifacts via existing Research Packet facility; identity, checksum, original filename, display title, printed attribution and page-location capture. No lossy silent OCR replacement. Export read-only NavigationFrame. Add tests for page furniture, unknown attribution and stale revisions.

B. Arrangement engine (pure deterministic pass): expose evidence-linked observations at unit / section / manuscript / corpus scopes. Preserve exact, structural, meta-functional recurrence as distinct grades. Compare links without forcing VEC_COMP or a fixed five-stage progression. A candidate must display the spans supporting it, counterexamples and an optional alternative reading.

C. Source intelligence (Rust owner, can proceed once A contract is frozen): SourceUse and SourceAlignment, text diff statuses retained/added/omitted/reordered/reworded/unresolved. Separate TEXTUAL / OPERATIONAL / SOURCE-INHERITED / EDITORIAL-VERIFIED / ARCHIVAL / UNRESOLVED lineage. No attribution of an inherited first-person anecdote to Rafael without proof.

D. Alongside UI (React owner, can mock against shared contract): select a span → Guide pane gives "what is visible / relationship / source / uncertainty" plus a jump-to-evidence action. Compare two passages at different scales. Source rail provenance badge in neutral styling. Sidecar only. The main editor is not modified by navigation. ASK/SUGGEST/PREVIEW/APPLY remains permission-gated.

E. Verification (independent gate): contract tests, offline import of sixteen PDFs, no-network checks, original checksum stability, zero unsolicited write to canonical Markdown, no source-image destruction, proper negative controls: filename vs printed title, page header vs rhetorical halt, at least-n arithmetic, no personal-biography laundering, a source comparison where function survives and one where it doesn't.

## First vertical slice
Import *Lessons From the Chicken Pen* and *The Temple of Education & Evangelism*. Show a direct local address on RK012; show a manuscript-level building→body correspondence on RK010; preserve Matthew 23:37 and 1 Corinthians 3:16–18 as running page framing, not auto-scored halts; one unresolved attribution and one optional meta-functional comparison. Opening an insight jumps to the original PDF page and highlights the retained span when location permits. Do NOT rewrite either source.

## Acceptance gate before broadening
- Immutable bytes and original spelling demonstrably preserved.
- All displayed claims trace to visible evidence; an absent field remains empty.
- One structural recurrence can be suggested, challenged and dismissed without editing the sermon.
- Inherited "I/we" is labeled textual voice rather than assigned to the archive owner.
- An invalid canonical NLM tag does not leak into UI as fact.
- Full app builds and current tests pass after integration; contract-only commits are not claimed as an app release.

## Source synthesis cautions
The uploaded sixteen-record meta synthesis is useful as a **single witness**; several hard census claims need audit. In particular *Don't Hide* has no explicit Scripture reference, the *Strong Delusion* page header is 2 Thessalonians 2:11–12 (not Matthew 23:37), and repeated page-top furniture does not itself establish rhetorical restarts. Its "exactly one engine in every manuscript" is a hypothesis, not a frozen system constraint.

## Ownership and merge gate
Changes must land on distinct branches from this frozen foundation. Track A schema + fixture is prerequisite for C and D runtime integration. CI green, original-artifact integrity, and contract-compatible mocks are required before any merge. No branch merges automatically.
