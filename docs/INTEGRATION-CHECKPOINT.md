# Integration checkpoint

Canonical commit: `b459b30bff4e0488ea79a9e1e437a5faf0d5123e`

This checkpoint merges the six verified backend tracks: reference parsing, canonical sermon AST, index/reconciliation, Typst/PDF export, structural linting, and the Tauri IPC/frontend bridge. Track and integration history remains preserved.

Verified baseline:

- `cargo build --workspace --locked`: pass
- Rust tests: 161 passed, 0 failed
- Jest tests: 70 passed, 0 failed
- `npm run build`: pass
- TypeScript: 12 known errors, with no new errors
- `typescript.ignoreBuildErrors`: enabled as recorded debt
- build-time ESLint validation: skipped as recorded debt
- lint/export IPC seams and startup reconciliation: linked

Must not regress:

- Rust and Jest pass counts cannot fall below the checkpoint minimums.
- TypeScript errors cannot exceed 12.
- React components consume `SermonBackend` through context rather than concrete adapters.
- Tauri, Typst/PDF, SQLite, reconciliation, and lint rule implementations remain behind their existing boundaries.
- The Next.js frontend remains a static export with no API routes or server actions.
- Shared contracts and canonical reference/directive semantics change only through an explicit integration decision.
