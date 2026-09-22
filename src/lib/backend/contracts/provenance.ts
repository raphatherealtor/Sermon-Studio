// ── Provenance contract (V1 — frozen) ────────────────────────────────────────
// TypeScript mirror of `crates/core/src/provenance.rs`.
//
// This file is the *transport contract* only. Rust owns the authoritative
// definitions and all decision logic; TypeScript mirrors the shapes so the UI
// can render provenance badges and attribution without re-deriving anything.
//
// The wire form of `ProvenanceClass` is kebab-case and MUST match the Rust
// serde representation exactly. Do not rename members without a version bump.

/** The five provenance classes. Wire form is kebab-case. */
export type ProvenanceClass =
  | 'your-archive'
  | 'biblical-study'
  | 'research-packet'
  | 'sermon-intelligence'
  | 'armarius';

/** All classes, in a stable order (mirrors `ProvenanceClass::ALL`). */
export const PROVENANCE_CLASSES: readonly ProvenanceClass[] = [
  'your-archive',
  'biblical-study',
  'research-packet',
  'sermon-intelligence',
  'armarius',
] as const;

/**
 * A provenance record attached to any surfaced content.
 * Mirrors the Rust `Provenance` DTO. Optional fields are omitted on the wire
 * when absent (Rust uses `skip_serializing_if = "Option::is_none"`).
 */
export interface Provenance {
  class: ProvenanceClass;
  sourceId?: string;
  sourceLabel: string;
  sourceVersion?: string;
  licenseCode?: string;
  attribution?: string;
  sermonId?: string;
  attachmentId?: string;
  page?: number;
  engineVersion?: string;
}

/**
 * **The hard invariant, mirrored for the UI.**
 * Returns `true` if and only if the class is `your-archive`.
 *
 * The UI may use this to decide whether to show an "authored by you" affordance,
 * but it MUST NOT be the only enforcement point — Rust enforces the invariant at
 * the data layer. This mirror exists for display only.
 */
export function isLifetimeCorpus(provenance: Pick<Provenance, 'class'>): boolean {
  return provenance.class === 'your-archive';
}
