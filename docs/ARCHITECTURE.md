# Sermon Studio Architecture Guardrails

## Frontend

The web client is a static Next.js/TypeScript application used as the UI build layer for a future Tauri 2 desktop application.

Required:
- `output: "export"`
- production artifact in `out/`
- fully usable browser preview with injected mock backend
- no server runtime required for product behavior
- no runtime network dependency

## Backend boundary

All product behavior must flow through an injected `SermonBackend` interface.

Concrete implementations:
- `MockSermonBackend`: deterministic browser-preview behavior
- `TauriSermonBackend`: thin IPC adapter only

React components must not directly import concrete backend implementations or fixtures.

## Rust/Tauri authority

Rust owns:
- Scripture/citation parsing
- canonical sermon AST validation
- structural lint logic
- SQLite/indexing
- filesystem reconciliation
- immutable export compilation
- Typst
- PDF generation

TypeScript may define transport codecs, typed contracts, presentation state, mocks, and UI orchestration. It must not recreate authoritative Rust logic.
