# Technical debt register

Checkpoint: `b459b30bff4e0488ea79a9e1e437a5faf0d5123e`

| Item | Owner | Status | Exit condition | Release impact |
| --- | --- | --- | --- | --- |
| 12 TypeScript errors | Frontend integration | Accepted baseline; CI rejects error 13 | `npm run type-check` reports zero errors and the baseline is reduced to zero | Does not block continued integration while the count stays at or below 12; close before release candidate |
| `typescript.ignoreBuildErrors: true` | Build/frontend | Temporary compatibility setting; independently guarded by `ci:typecheck` | Remove after the TypeScript baseline reaches zero and a normal build validates types | Does not block the current checkpoint with CI enabled; blocks release if the independent type gate is absent |
| Build skips ESLint | Frontend tooling | Existing behavior; no lint baseline is claimed | Replace the legacy `next lint` script with a verified ESLint command, then enable it as a separate gate | Does not block this checkpoint; close before release candidate |

Baseline reductions are welcome. Never increase a baseline to make CI pass without an explicit checkpoint decision.
