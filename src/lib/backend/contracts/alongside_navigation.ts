/**
 * Alongside evidence foundation v0.1.
 * Additive contract: NOT a new NLM ontology or a mutable sermon AST.
 * All evidence is tied to an immutable artifact revision and an inspectable location.
 * Analysis records are suggestions; APPLY is a separate explicit editor action.
 */
export const NAVIGATION_SCHEMA_VERSION = '0.1' as const;
export type EvidenceStatus = 'direct' | 'structurally-derived' | 'candidate' | 'unresolved';
export type EvidenceScope = 'unit' | 'section' | 'manuscript' | 'corpus';
export type ProvenanceStatus =
  | 'visible-in-artifact'
  | 'externally-verified'
  | 'candidate'
  | 'unresolved';
export type LineageRole =
  | 'textual' | 'operational' | 'source-inherited'
  | 'editorial-verified' | 'archival' | 'unresolved';
export type SourceKind =
  | 'scripture' | 'historical-sermon' | 'devotional' | 'commentary'
  | 'lexical' | 'anecdote' | 'hymn-poem' | 'performance'
  | 'historical-claim' | 'bibliographic-apparatus' | 'unknown';
export type SourceUseKind =
  | 'quotation' | 'paraphrase' | 'allusion' | 'illustration'
  | 'comparison' | 'adaptation' | 'reference' | 'apparatus';
export type ArrangementFunction =
  | 'anchor' | 'render' | 'develop' | 'distinguish'
  | 'reposition' | 'handoff' | 'relate' | 'traverse' | 'other';
export type SourceSpanRelationship =
  | 'retained' | 'added' | 'omitted' | 'reordered'
  | 'reworded' | 'unresolved';

/** OCR coordinates can be absent; the system must not invent them. */
export interface ArtifactLocation {
  page?: number;
  paragraph?: number;
  startOffset?: number;
  endOffset?: number;
  quotation?: string;
  /** e.g. source PDF page image, OCR text, or user-verified transcription. */
  carrier?: string;
}
export interface ArtifactIdentity {
  artifactId: string;
  revisionHash: string;
  originalFilename: string;
  displayedTitle?: string;
  /** Filename, on-page title, and byline remain separate fields. */
  printedAttribution?: string[];
  sourceFileChecksum?: string;
}
export interface NavigationEvidence {
  id: string;
  artifactId: string;
  revisionHash: string;
  location: ArtifactLocation;
  originalSpan: string;
  scope: EvidenceScope;
  status: EvidenceStatus;
  provenance: ProvenanceStatus;
  /** Free text, never used to silently assign canonical NLM tags. */
  observation: string;
  operation?: string;
  carrier?: string;
  relationId?: string;
  /** Present only when a claim compares like with like. */
  comparisonDimension?: string;
  alternativeReading?: string;
  unresolvedReason?: string;
}
export interface ArrangementObservation {
  id: string;
  artifactId: string;
  revisionHash: string;
  function: ArrangementFunction;
  description: string;
  evidenceIds: string[];
  status: EvidenceStatus;
  /** An order is a candidate observed order, never a universal sermon template. */
  proposedOrder?: number;
  alternativeReading?: string;
}
export interface SourceUse {
  id: string;
  artifactId: string;
  revisionHash: string;
  evidenceLocation: ArtifactLocation;
  sourceType: SourceKind;
  sourceIdentity?: string;
  sourceLocation?: string;
  usage: SourceUseKind;
  operationEvidenceIds: string[];
  provenanceStatus: ProvenanceStatus;
  lineageRole: LineageRole;
  /** Textual voice is not a verified personal recollection of the archive owner. */
  textualSpeaker?: string;
  notes?: string;
}
export interface SourceAlignment {
  id: string;
  artifactId: string;
  revisionHash: string;
  sourceUseId: string;
  retainedLocation: ArtifactLocation;
  comparedSourceLocation?: string;
  relation: SourceSpanRelationship;
  retainedText: string;
  comparedSourceText?: string;
  verifiedBy?: string;
  status: ProvenanceStatus;
}
export interface AnalysisWitness {
  id: string;
  modelOrAnalyst: string;
  recordId: string;
  method?: string;
  /** Sixteen files from one model are still one analytical witness. */
  independenceGroup: string;
}
export interface NavigationFrame {
  schemaVersion: typeof NAVIGATION_SCHEMA_VERSION;
  artifact: ArtifactIdentity;
  witnesses: AnalysisWitness[];
  evidence: NavigationEvidence[];
  arrangements: ArrangementObservation[];
  sourceUses: SourceUse[];
  sourceAlignments: SourceAlignment[];
  /** Never canonical sermon Markdown or an APPLY instruction. */
  canonicalMutationAllowed: false;
}
export function checkNavigationFrame(frame: NavigationFrame): string[] {
  const problems: string[] = [];
  const ids = new Set<string>();
  for (const evidence of frame.evidence) {
    if (ids.has(evidence.id)) problems.push(`duplicate evidence id: ${evidence.id}`);
    ids.add(evidence.id);
    if (evidence.artifactId !== frame.artifact.artifactId ||
        evidence.revisionHash !== frame.artifact.revisionHash) {
      problems.push(`revision mismatch: ${evidence.id}`);
    }
    if (!evidence.originalSpan.trim()) problems.push(`missing original span: ${evidence.id}`);
  }
  for (const a of frame.arrangements) {
    if (a.artifactId !== frame.artifact.artifactId ||
        a.revisionHash !== frame.artifact.revisionHash) problems.push(`arrangement revision mismatch: ${a.id}`);
    for (const id of a.evidenceIds) if (!ids.has(id)) problems.push(`missing arrangement evidence: ${a.id}/${id}`);
  }
  for (const s of frame.sourceUses) {
    if (s.artifactId !== frame.artifact.artifactId ||
        s.revisionHash !== frame.artifact.revisionHash) problems.push(`source use revision mismatch: ${s.id}`);
    for (const id of s.operationEvidenceIds) if (!ids.has(id)) problems.push(`missing source evidence: ${s.id}/${id}`);
    if (s.lineageRole === 'editorial-verified' && s.provenanceStatus !== 'externally-verified') {
      problems.push(`unverified editorial assignment: ${s.id}`);
    }
  }
  const sourceIds = new Set(frame.sourceUses.map(s => s.id));
  for (const a of frame.sourceAlignments) {
    if (!sourceIds.has(a.sourceUseId)) problems.push(`missing alignment source: ${a.id}`);
    if (a.status === 'externally-verified' && !a.comparedSourceText) problems.push(`verified alignment lacks compared text: ${a.id}`);
  }
  if (frame.canonicalMutationAllowed !== false) problems.push('canonical mutation forbidden');
  return problems;
}
