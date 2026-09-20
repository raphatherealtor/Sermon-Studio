# Sermon Studio — Frontend

A portable, static-exportable frontend for an offline expository sermon writing environment. Built with Next.js (static export), TypeScript strict mode, TipTap, and Zustand. Designed for later embedding inside a Tauri 2.0 / WebKitGTK Linux desktop shell.

---

## Running the Frontend

```bash
npm install
npm run dev          # development server on port 4028
npm run build        # production static export → out/
npm run type-check   # TypeScript strict check (no emit)
npm test             # run directive codec + integration tests
```

The development server uses `MockSermonBackend` automatically. No backend, database, or network connection is required.

---

## Production Static Build

```bash
npm run build
```

Output: `out/` directory — a pure static client bundle.

- No server runtime required
- No API routes
- No SSR dependency
- Suitable for direct embedding as Tauri frontend assets
- No external network calls at runtime
- No remote fonts, CDN assets, or analytics

Configured via `next.config.mjs`:
```js
output: 'export'
images: { unoptimized: true }
```

---

## Architecture

### Three-Panel "Preacher's Desk"

```
┌─────────────────┬──────────────────────────┬──────────────────┐
│  Archive Rail   │     Sermon Editor         │   Study Rail     │
│  (left)         │     (center)              │   (right)        │
│                 │                           │                  │
│  - Search       │  - TipTap editor          │  - Passage       │
│  - Sermon list  │  - Directive blocks       │  - Strong's      │
│  - Filters      │  - Lint panel             │  - Cross-refs    │
│  - New sermon   │  - Conflict banner        │  - History       │
│  - Context menu │  - Word count / autosave  │  - Fatigue       │
└─────────────────┴──────────────────────────┴──────────────────┘
```

### Backend Adapter Contract

All backend/native behavior flows through a single injected interface:

```typescript
// src/lib/backend/SermonBackend.ts
interface SermonBackend {
  listSermons(): Promise<SermonSummary[]>
  loadSermon(id: string): Promise<SermonDocument>
  saveSermon(doc: SermonDocument): Promise<SaveResult>
  lintSermon(doc: SermonDocument): Promise<LintFinding[]>
  getPassage(reference: string): Promise<PassageResult>
  getStrongs(id: string): Promise<StrongsEntry>
  exportSermon(request: ExportRequest): Promise<ExportResult>
  resolveConflict(request: ConflictResolution): Promise<SaveResult>
  // ... 30+ methods — see SermonBackend.ts for full contract
}
```

**Rule:** React components may only import:
- `useBackend()` from `BackendContext`
- Types from `types.ts`

React components must **never** import `MockSermonBackend`, `TauriSermonBackend`, or fixture files directly.

### MockSermonBackend

`src/lib/backend/MockSermonBackend.ts`

- Powers the browser preview
- Implements every `SermonBackend` method with deterministic stub data
- 14 mock sermons covering: normal, long, incomplete, lint-heavy, conflict, missing file, illustration reuse, unknown directive, archived, export history
- Delegates codec tests to the shared `directiveCodec.ts`
- **Never import in React components** — injected via `BackendProvider`

### TauriSermonBackend

`src/lib/backend/TauriSermonBackend.ts`

- Thin IPC adapter for the future Rust/Tauri backend
- Every method maps 1:1 to one Tauri IPC command
- No business logic
- Dynamically imports `@tauri-apps/api` only when instantiated (keeps browser bundle clean)
- **Never import in React components** — injected via `BackendProvider`

### Backend Selection

```typescript
// src/app/layout.tsx or app entry point
import { BackendProvider } from '@/lib/backend/BackendContext';
import { MockSermonBackend } from '@/lib/backend/MockSermonBackend';

<BackendProvider backend={new MockSermonBackend()}>
  <App />
</BackendProvider>
```

For Tauri: replace `MockSermonBackend` with `TauriSermonBackend` at this single injection point. No component changes required.

---

## Directive Transport Codec

`src/editor/codec/directiveCodec.ts`

Shared frontend infrastructure for parsing and serializing directive Markdown.

**Contract:**
- Transport-only: no Scripture normalization, no AST validation, no lint rules
- Unknown directives round-trip losslessly (name + attributes + body preserved)
- Known directives receive specialized editor rendering

**Known directives:** `big-idea`, `application`, `illustration`, `note`, `scripture`, `movement`, `warrant`

**Unknown directives** (e.g. `:::custom-block{foo="bar"}`) are preserved as opaque transport data and must survive `load → editor state → save` without semantic loss.

**Tests:** `src/editor/codec/__tests__/directiveCodec.test.ts`

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl+P` | Open command palette |
| `Cmd/Ctrl+K` | Focus archive search |
| `Cmd/Ctrl+S` | Save sermon |
| `Cmd/Ctrl+N` | New sermon |
| `Cmd/Ctrl+⇧L` | Run linter |
| `Escape` | Close command palette / overlays |

---

## Screens

| Route | Description |
|-------|-------------|
| `/` | Preacher's Desk (3-panel workspace) |
| `/export-screen` | Export configurator |
| `/archive-overview` | Archive dashboard with charts |
| `/settings-index-management` | Settings, index management, developer tools |
| `/codec-test` | Directive transport codec test (dev tool) |

---

## Rust / Native Responsibilities

The following are **outside TypeScript** and must not be reimplemented here:

- Scripture citation parsing (KJV-era, Roman numeral, irregular)
- Canonical sermon AST validation
- Structural lint rule computation
- SQLite persistence and indexing
- Filesystem reconciliation engine
- Typst rendering
- PDF generation
- Canonical export compilation

TypeScript defines types, request/response contracts, display states, transport codecs, UI validation, and mocks — but does not recreate authoritative logic.

---

## Project Structure

```
src/
├── app/
│   ├── page.tsx                          # Root → Preacher's Desk
│   ├── components/
│   │   ├── SermonEditorWorkspace.tsx     # 3-panel layout orchestrator
│   │   └── editor/
│   │       ├── ArchiveRail.tsx           # Left: sermon list + search
│   │       ├── EditorPanel.tsx           # Center: TipTap + lint + conflict
│   │       ├── TipTapEditor.tsx          # TipTap editor instance
│   │       └── StudyRail.tsx             # Right: passage/strongs/xref
│   ├── export-screen/                    # Export configurator
│   ├── archive-overview/                 # Archive dashboard
│   ├── settings-index-management/        # Settings + index management
│   └── codec-test/                       # Directive codec dev tool
├── components/
│   ├── AppLayout.tsx                     # Shell: sidebar + keyboard handlers
│   ├── CommandPalette.tsx                # Cmd+P command palette
│   └── Sidebar.tsx                       # Navigation sidebar
├── editor/
│   └── codec/
│       ├── directiveCodec.ts             # Directive transport codec
│       └── __tests__/
│           └── directiveCodec.test.ts    # Codec unit tests
├── lib/
│   ├── backend/
│   │   ├── SermonBackend.ts              # Interface contract
│   │   ├── MockSermonBackend.ts          # Browser preview implementation
│   │   ├── TauriSermonBackend.ts         # Tauri IPC adapter stub
│   │   ├── BackendContext.tsx            # React context + useBackend()
│   │   └── types.ts                      # All domain types
│   └── store/
│       └── editorStore.ts                # Zustand editor state
└── __tests__/
    └── integration/
        └── frontend.test.tsx             # Frontend integration tests
```

---

## Static / Offline Verification Checklist

- [x] `output: 'export'` enabled in `next.config.mjs`
- [x] No API routes
- [x] No Server Actions
- [x] No SSR dependency
- [x] No runtime network calls
- [x] No remote fonts
- [x] No CDN assets
- [x] No analytics or telemetry
- [x] No hosted persistence
- [x] No React component imports `MockSermonBackend` or `TauriSermonBackend` directly
- [x] No React component imports fixture data directly
- [x] Unknown directives survive transport round-trip
- [x] Conflict handling is inline in the editor workspace (not a separate route)
- [x] Tauri imports are dynamically isolated inside `TauriSermonBackend`

---

Built with [Rocket.new](https://rocket.new)