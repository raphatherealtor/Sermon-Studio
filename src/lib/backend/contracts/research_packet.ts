// ── Research Packet contract (V1) ────────────────────────────────────────────
// TypeScript mirror of `crates/core/src/research_packet.rs`.
//
// Transport contract only. Rust owns content-addressing, path confinement,
// manifest validation, and extraction. The UI reads these shapes to render the
// packet panel and attribution.

import type { ProvenanceClass } from './provenance';

/** Current packet manifest version. */
export const PACKET_VERSION = '1';

/** Vault-relative directory that holds all research packets. */
export const ATTACHMENTS_DIR = '.sermon-studio/attachments';

/** Extraction status of an attachment. Wire form is kebab-case. */
export type ExtractionStatus = 'ok' | 'no-text' | 'failed' | 'not-attempted';

/** A single extracted page of text. */
export interface ExtractedPage {
  page: number;
  text: string;
}

/** Where an attachment was imported from. */
export interface ImportProvenance {
  importedFrom: string;
  importedAt: string;
}

/** A single attachment within a research packet. */
export interface ResearchAttachment {
  id: string;
  originalFilename: string;
  storedFilename: string;
  title?: string;
  author?: string;
  source?: string;
  dateAdded: string;
  mimeType: string;
  byteSize: number;
  pageCount?: number;
  checksum: string;
  extraction: ExtractionStatus;
  userNotes?: string;
  importProvenance: ImportProvenance;
  /** Always 'research-packet'. */
  provenanceClass: ProvenanceClass;
}

/** The packet manifest, stored at `manifest.json`. */
export interface ResearchPacketManifest {
  packetVersion: string;
  sermonId: string;
  attachments: ResearchAttachment[];
}
