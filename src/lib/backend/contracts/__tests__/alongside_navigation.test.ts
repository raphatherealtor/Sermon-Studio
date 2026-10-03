import {
  NAVIGATION_SCHEMA_VERSION,
  checkNavigationFrame,
  type NavigationFrame,
} from '../alongside_navigation';

const fixture = (): NavigationFrame => ({
  schemaVersion: NAVIGATION_SCHEMA_VERSION,
  artifact: { artifactId: 'RK012', revisionHash: 'immutable-test-revision', originalFilename: 'Lessons From_.pdf', displayedTitle: 'Lessons From the Chicken Pen' },
  witnesses: [
    { id: 'vibe-rk012', modelOrAnalyst: 'Vibe', recordId: 'rk012-vibe', independenceGroup: 'vibe' },
    { id: 'zcode-rk012', modelOrAnalyst: 'ZCode', recordId: 'rk012-zcode', independenceGroup: 'zcode' },
  ],
  evidence: [{
    id: 'e1', artifactId: 'RK012', revisionHash: 'immutable-test-revision',
    location: { page: 6, quotation: 'How about you?' }, originalSpan: 'How about you?',
    scope: 'unit', status: 'direct', provenance: 'visible-in-artifact', observation: 'Direct audience address',
  }],
  arrangements: [{
    id: 'a1', artifactId: 'RK012', revisionHash: 'immutable-test-revision',
    function: 'reposition', description: 'Observation to audience address',
    evidenceIds: ['e1'], status: 'structurally-derived',
  }],
  sourceUses: [{
    id: 's1', artifactId: 'RK012', revisionHash: 'immutable-test-revision',
    evidenceLocation: { page: 1 }, sourceType: 'scripture', sourceIdentity: 'Matthew 23:37',
    usage: 'quotation', operationEvidenceIds: [], provenanceStatus: 'visible-in-artifact', lineageRole: 'textual',
  }],
  sourceAlignments: [],
  canonicalMutationAllowed: false,
});

describe('Alongside NavigationFrame contract', () => {
  it('validates an anchored, non-mutating frame', () => {
    expect(checkNavigationFrame(fixture())).toEqual([]);
  });
  it('refuses invented or missing evidence joins', () => {
    const frame = fixture();
    frame.arrangements[0].evidenceIds.push('missing');
    expect(checkNavigationFrame(frame)).toContain('missing arrangement evidence: a1/missing');
  });
  it('refuses stale revision references', () => {
    const frame = fixture();
    frame.evidence[0].revisionHash = 'older';
    expect(checkNavigationFrame(frame)).toContain('revision mismatch: e1');
  });
  it('does not grant editorial authorship from on-page text alone', () => {
    const frame = fixture();
    frame.sourceUses[0].lineageRole = 'editorial-verified';
    expect(checkNavigationFrame(frame)).toContain('unverified editorial assignment: s1');
  });
  it('rejects externally verified alignments with no source comparison', () => {
    const frame = fixture();
    frame.sourceAlignments.push({
      id:'align1', artifactId:'RK012',revisionHash:'immutable-test-revision',
      sourceUseId:'s1', retainedLocation:{ page:1 }, relation:'reworded',
      retainedText:'test', status:'externally-verified',
    });
    expect(checkNavigationFrame(frame)).toContain('verified alignment lacks compared text: align1');
  });
  it('preserves analyst independence groups', () => {
    const frame = fixture();
    expect(new Set(frame.witnesses.map(w => w.independenceGroup)).size).toBe(2);
  });
});
