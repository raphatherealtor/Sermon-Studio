# Sermon Studio

Sermon Studio is a local-first desktop sermon research, writing, linting, archive, and export application.

## Frontend architecture

- Next.js + TypeScript
- Static export only: `output: "export"`
- Production bundle: `out/`
- Intended for later Tauri 2 embedding
- No SSR dependency
- No API routes or Server Actions
- No hosted database, authentication, analytics, telemetry, or runtime cloud dependency
- No remote fonts/CDNs
- All product/backend behavior flows through the injected `SermonBackend` interface
- Browser preview uses `MockSermonBackend`
- Desktop runtime uses `TauriSermonBackend`, isolated behind the adapter

## Rust-owned responsibilities

The frontend must not reimplement these authoritative systems:

- Scripture citation parsing
- KJV-era Roman numeral / irregular citation parsing
- Canonical sermon AST validation
- Structural lint computation
- SQLite persistence/indexing
- Filesystem reconciliation
- Typst rendering
- PDF generation
- Canonical export compilation

## Builder rule

Rocket/Lovable may build frontend surfaces, workflows, typed adapter contracts, mocks, and presentation logic. Rust/Tauri remains authoritative for parsing, indexing, filesystem, linting, and export internals.

This repository is the canonical source repository for the Sermon Studio build.
