# Recommended next build wave

## Objective

Turn the verified backend seams into one production-honest native workflow while retiring the bounded frontend contract drift. Keep the Rust core and shared contracts authoritative.

## Priority 0: native composition and vertical proof

1. Add one application composition point that selects `TauriSermonBackend` in the native runtime and `MockSermonBackend` only for browser preview/tests.
2. Prove open, edit, conflict-safe save, lint, snapshot, and PDF export through the real Tauri adapter in one focused native integration test.
3. Keep React components on `SermonBackend`; do not add direct Tauri imports.

Exit: the packaged native application completes the vertical workflow without fixture behavior or duplicate frontend logic.

## Priority 1: close the 12-error contract drift

Resolve the current diagnostics as contract-alignment slices:

- lint fixture fields and export request options
- Strong's, sermon word-count, and conflict metadata shapes
- conflict strategy and export format/mode unions
- the separate `ui` package's declared Tauri and editor dependencies

Reduce `typescriptErrorMaximum` with every landed slice. Remove `ignoreBuildErrors` when the baseline reaches zero.

## Priority 2: release-path confidence

1. Add a packaged-app smoke test using a temporary vault and deterministic fixture sermon.
2. Verify startup reconciliation, external-file conflict detection, private-note exclusion, and exported PDF validity in that smoke path.
3. Record platform packaging prerequisites only after the native vertical workflow passes.

## External agent inputs

No DeepSeek or Kimi reports were present in this checkout or request. When supplied, classify their findings into: release blocker, safe quick fix, medium-priority debt, or next-wave scope. Accept only findings reproduced against the canonical checkpoint.
