// Powers the browser preview. All data is deterministic stubs.
// NEVER import this directly in React components — use BackendContext.

import type { SermonBackend } from './SermonBackend';
import type {
  SermonSummary,
  SermonDocument,
  CreateSermonRequest,
  RenameSermonRequest,
  DuplicateSermonRequest,
  SaveResult,
  SearchResult,
  SearchFilters,
  ReferenceMatch,
  LintFinding,
  PassageResult,
  StrongsEntry,
  CrossReference,
  PreachedResult,
  IndexOperationResult,
  IndexStatus,
  ArchiveStats,
  IllustrationFatigueResult,
  ExportRequest,
  ExportResult,
  ExportSnapshot,
  CreateExportSnapshotRequest,
  ConflictResolution,
  DiffPreparationResult,
  MergePreparationResult,
  FilesystemReconciliationStatus,
  RecoveryResult,
  ReconnectRequest,
  RevealFileRequest,
  AppSettings,
  CodecRoundTripResult,
  IntelligenceResult,
  IntelligenceInsight,
  IntelligenceEvidence,
  Provenance, ChainStudyResult } from './types';
// Fix 5: Import the shared frontend codec — MockSermonBackend provides fixtures only,
// not the codec implementation.
import { testRoundTrip } from '@/editor/codec/directiveCodec';
// Track O: research-packet transport shapes (fixture-only mock; Rust owns the store).
import type {
  AttachResearchFileRequest,
  ExtractedPage,
  OpenResearchFileResult,
  ResearchAttachment,
  UpdateResearchMetadataRequest,
} from './contracts/research_packet';

const delay = (ms = 220) => new Promise<void>((r) => setTimeout(r, ms));

// ── Sermon Intelligence fixtures (deterministic; Track J computes) ──────────
// These are static presentation fixtures only. No scoring/search/correlation
// logic lives here — every value is a literal that mirrors Track J's DTOs.

const INTELLIGENCE_ENGINE_VERSION = 'mock-1.0.0';
const INTELLIGENCE_GENERATED_AT = '2026-09-20T12:00:00Z';

function evidence(
  kind: string,
  label: string,
  value: string,
  opts: { weight?: number; sermonIds?: string[]; references?: string[]; provenanceClass?: Provenance['class'] } = {},
): IntelligenceEvidence {
  return {
    kind,
    label,
    value,
    weight: opts.weight ?? 1,
    sermonIds: opts.sermonIds ?? [],
    references: opts.references ?? [],
    provenance: {
      class: opts.provenanceClass ?? 'your-archive',
      sourceLabel: opts.provenanceClass === 'biblical-study' ? 'Biblical Study' : 'Your Archive',
    },
  };
}

function insight(
  id: string,
  kind: IntelligenceInsight['kind'],
  title: string,
  summary: string,
  opts: { score?: number; evidence?: IntelligenceEvidence[]; relatedSermonIds?: string[] } = {},
): IntelligenceInsight {
  return {
    id,
    kind,
    title,
    summary,
    score: opts.score ?? 0,
    evidence: opts.evidence ?? [],
    relatedSermonIds: opts.relatedSermonIds ?? [],
  };
}

/** Archive-only insights (no biblical-study evidence), for the no-canon.db case. */
function archiveOnlyInsights(): IntelligenceInsight[] {
  return [
    insight('rel-gospel-of-john', 'related-sermon', 'Related to your Gospel of John series', 'These sermons share the Gospel of John series.', {
      score: 70,
      relatedSermonIds: ['sermon-002', 'sermon-003', 'sermon-013', 'sermon-014'],
      evidence: [
        evidence('series', 'Same series', 'Gospel of John', { weight: 3, sermonIds: ['sermon-002', 'sermon-003', 'sermon-013', 'sermon-014'], provenanceClass: 'your-archive' }),
      ],
    }),
    insight('ill-pattern-bread', 'illustration-pattern', 'Illustration: bread and hunger', 'The bread/hunger illustration appears in 2 sermons.', {
      score: 41,
      relatedSermonIds: ['sermon-009'],
      evidence: [
        evidence('illustration', 'Illustration reuse', 'bread / hunger (2 uses)', { weight: 2, sermonIds: ['sermon-009'], provenanceClass: 'your-archive' }),
      ],
    }),
  ];
}

/** Full fixture for a subject sermon, with archive + biblical-study evidence. */
function fullInsights(sermonId: string, reference: string): IntelligenceResult {
  return {
    engineVersion: INTELLIGENCE_ENGINE_VERSION,
    generatedAt: INTELLIGENCE_GENERATED_AT,
    subjectSermonId: sermonId,
    subjectReference: reference,
    biblicalDataAvailable: true,
    insights: [
      insight('rel-gospel-of-john', 'related-sermon', 'Related to your Gospel of John series', 'These sermons share the Gospel of John series and overlapping passages.', {
        score: 87,
        relatedSermonIds: ['sermon-002', 'sermon-003', 'sermon-013', 'sermon-014'],
        evidence: [
          evidence('series', 'Same series', 'Gospel of John', { weight: 3, sermonIds: ['sermon-002', 'sermon-003', 'sermon-013', 'sermon-014'], provenanceClass: 'your-archive' }),
          evidence('shared-reference', 'Shared reference', 'John 6:35', { weight: 2, sermonIds: ['sermon-002'], references: ['John 6:35'], provenanceClass: 'biblical-study' }),
        ],
      }),
      insight('ref-overlap-john', 'reference-overlap', '3 shared supporting references', 'Your archive contains sermons that cite the same supporting passages.', {
        score: 64,
        relatedSermonIds: ['sermon-002', 'sermon-003', 'sermon-013'],
        evidence: [
          evidence('shared-reference', 'Shared reference', 'John 6:35', { weight: 2, sermonIds: ['sermon-002'], references: ['John 6:35'], provenanceClass: 'biblical-study' }),
          evidence('shared-reference', 'Shared reference', 'John 1:1–14', { weight: 1, sermonIds: ['sermon-002', 'sermon-013'], references: ['John 1:1'], provenanceClass: 'biblical-study' }),
        ],
      }),
      insight('big-idea-bread-life', 'big-idea-overlap', 'Big Idea overlap: life and sustenance', 'These sermons share Big Idea terms: life, bread, sustain.', {
        score: 52,
        relatedSermonIds: ['sermon-002', 'sermon-014'],
        evidence: [
          evidence('big-idea-term', 'Shared Big Idea terms', 'life, bread', { weight: 2, sermonIds: ['sermon-002', 'sermon-014'], provenanceClass: 'your-archive' }),
        ],
      }),
      insight('series-gospel-of-john', 'series-overlap', 'Part of the Gospel of John series', '4 sermons belong to the same series.', {
        score: 70,
        relatedSermonIds: ['sermon-002', 'sermon-003', 'sermon-013', 'sermon-014'],
        evidence: [
          evidence('series', 'Same series', 'Gospel of John', { weight: 3, sermonIds: ['sermon-002', 'sermon-003', 'sermon-013', 'sermon-014'], provenanceClass: 'your-archive' }),
        ],
      }),
      insight('ill-pattern-bread', 'illustration-pattern', 'Illustration: bread and hunger', 'The bread/hunger illustration appears in 2 sermons.', {
        score: 41,
        relatedSermonIds: ['sermon-009'],
        evidence: [
          evidence('illustration', 'Illustration reuse', 'bread / hunger (2 uses)', { weight: 2, sermonIds: ['sermon-009'], provenanceClass: 'your-archive' }),
        ],
      }),
      insight('struct-3-movements', 'structure-overlap', 'Structure overlap: 3 movements', 'Mechanically measured: these sermons share a 3-movement outline.', {
        score: 38,
        relatedSermonIds: ['sermon-002', 'sermon-003'],
        evidence: [
          evidence('structure', 'Outline shape', '3 movements, 1 application', { weight: 1, sermonIds: ['sermon-002', 'sermon-003'], provenanceClass: 'your-archive' }),
        ],
      }),
      insight('history-john-10', 'passage-history', 'You preached John 10 once', 'Preaching history for the related passage John 10:11–18.', {
        score: 0,
        relatedSermonIds: ['sermon-003'],
        evidence: [
          evidence('passage-history', 'Preached', 'John 10:11–18 — 1 time (2026-09-07)', { weight: 3, sermonIds: ['sermon-003'], provenanceClass: 'your-archive' }),
        ],
      }),
    ],
  };
}

function passageHistoryInsights(reference: string): IntelligenceResult {
  return {
    engineVersion: INTELLIGENCE_ENGINE_VERSION,
    generatedAt: INTELLIGENCE_GENERATED_AT,
    subjectReference: reference,
    biblicalDataAvailable: true,
    insights: [
      insight('history-fixture', 'passage-history', `You preached ${reference} in your archive`, 'Preaching history for this passage.', {
        score: 0,
        relatedSermonIds: ['sermon-003', 'sermon-004'],
        evidence: [
          evidence('passage-history', 'Preached', `${reference} — 2 times (2026-09-07, 2026-08-25)`, { weight: 3, sermonIds: ['sermon-003', 'sermon-004'], provenanceClass: 'your-archive' }),
        ],
      }),
    ],
  };
}

// ── Mock sermon list ──────────────────────────────────────────────────────────

const MOCK_SERMONS: SermonSummary[] = [
  {
    id: 'sermon-001',
    title: 'The Bread of Life',
    scripture: 'John 6:35–51',
    series: 'Gospel of John',
    status: 'in-progress',
    wordCount: 3412,
    createdAt: '2026-09-01T09:00:00Z',
    updatedAt: '2026-09-19T21:45:00Z',
    preachedOn: null,
    tags: ['incarnation', 'faith', 'sustenance'],
    isPinned: true,
    sourcePath: '/home/preacher/sermons/gospel-of-john/bread-of-life.md',
    fsState: 'local-dirty',
  },
  {
    id: 'sermon-002',
    title: 'Light of the World',
    scripture: 'John 8:12–20',
    series: 'Gospel of John',
    status: 'reviewed',
    wordCount: 2987,
    createdAt: '2026-08-20T08:00:00Z',
    updatedAt: '2026-09-10T14:30:00Z',
    preachedOn: null,
    tags: ['light', 'testimony', 'truth'],
    sourcePath: '/home/preacher/sermons/gospel-of-john/light-of-the-world.md',
    fsState: 'clean',
  },
  {
    id: 'sermon-003',
    title: 'The Good Shepherd',
    scripture: 'John 10:11–18',
    series: 'Gospel of John',
    status: 'preached',
    wordCount: 3105,
    createdAt: '2026-08-01T10:00:00Z',
    updatedAt: '2026-09-07T09:00:00Z',
    preachedOn: '2026-09-07',
    tags: ['shepherd', 'sacrifice', 'protection'],
    sourcePath: '/home/preacher/sermons/gospel-of-john/good-shepherd.md',
    fsState: 'clean',
  },
  {
    id: 'sermon-004',
    title: 'Justification by Faith Alone',
    scripture: 'Romans 3:21–31',
    series: 'Romans: The Gospel Unpacked',
    status: 'preached',
    wordCount: 4102,
    createdAt: '2026-07-10T08:30:00Z',
    updatedAt: '2026-08-25T11:00:00Z',
    preachedOn: '2026-08-25',
    tags: ['justification', 'faith', 'law'],
    sourcePath: '/home/preacher/sermons/romans/justification.md',
    fsState: 'clean',
  },
  {
    id: 'sermon-005',
    title: 'No Condemnation',
    scripture: 'Romans 8:1–11',
    series: 'Romans: The Gospel Unpacked',
    status: 'preached',
    wordCount: 3780,
    createdAt: '2026-07-20T09:00:00Z',
    updatedAt: '2026-09-01T10:00:00Z',
    preachedOn: '2026-09-01',
    tags: ['condemnation', 'Spirit', 'freedom'],
    sourcePath: '/home/preacher/sermons/romans/no-condemnation.md',
    fsState: 'disk-changed',
    hasConflict: true,
  },
  {
    id: 'sermon-006',
    title: 'The Suffering Servant',
    scripture: 'Isaiah 53:1–12',
    series: 'Messianic Prophecies',
    status: 'preached',
    wordCount: 4450,
    createdAt: '2026-06-01T08:00:00Z',
    updatedAt: '2026-06-28T09:00:00Z',
    preachedOn: '2026-06-29',
    tags: ['atonement', 'prophecy', 'suffering'],
    sourcePath: '/home/preacher/sermons/messianic/suffering-servant.md',
    fsState: 'clean',
  },
  {
    id: 'sermon-007',
    title: 'The Armor of God',
    scripture: 'Ephesians 6:10–20',
    series: 'Ephesians: In the Heavenlies',
    status: 'preached',
    wordCount: 3620,
    createdAt: '2026-05-10T08:00:00Z',
    updatedAt: '2026-06-01T09:00:00Z',
    preachedOn: '2026-06-01',
    tags: ['spiritual warfare', 'armor', 'prayer'],
    sourcePath: '/home/preacher/sermons/ephesians/armor-of-god.md',
    fsState: 'clean',
  },
  {
    id: 'sermon-008',
    title: 'Blessed Are the Pure in Heart',
    scripture: 'Matthew 5:8',
    series: 'The Beatitudes',
    status: 'draft',
    wordCount: 812,
    createdAt: '2026-09-15T10:00:00Z',
    updatedAt: '2026-09-18T16:00:00Z',
    preachedOn: null,
    tags: ['beatitudes', 'purity', 'heart'],
    sourcePath: '/home/preacher/sermons/beatitudes/pure-in-heart.md',
    fsState: 'local-dirty',
  },
  {
    id: 'sermon-009',
    title: 'The Prodigal Son Returns',
    scripture: 'Luke 15:11–32',
    series: null,
    status: 'archived',
    wordCount: 3890,
    createdAt: '2025-12-01T09:00:00Z',
    updatedAt: '2026-01-05T10:00:00Z',
    preachedOn: '2026-01-05',
    tags: ['grace', 'repentance', 'father'],
    sourcePath: '/home/preacher/sermons/archive/prodigal-son.md',
    fsState: 'clean',
  },
  {
    id: 'sermon-010',
    title: 'Faith Without Works Is Dead',
    scripture: 'James 2:14–26',
    series: 'James: Practical Christianity',
    status: 'archived',
    wordCount: 3200,
    createdAt: '2025-10-15T09:00:00Z',
    updatedAt: '2025-11-10T10:00:00Z',
    preachedOn: '2025-11-10',
    tags: ['faith', 'works', 'sanctification'],
    sourcePath: '/home/preacher/sermons/archive/faith-works.md',
    fsState: 'clean',
  },
  {
    id: 'sermon-011',
    title: 'The New Covenant',
    scripture: 'Jeremiah 31:31–34',
    series: 'Messianic Prophecies',
    status: 'preached',
    wordCount: 3340,
    createdAt: '2026-04-01T09:00:00Z',
    updatedAt: '2026-04-27T10:00:00Z',
    preachedOn: '2026-04-27',
    tags: ['covenant', 'law', 'grace'],
    sourcePath: '/home/preacher/sermons/messianic/new-covenant.md',
    fsState: 'clean',
  },
  {
    id: 'sermon-012',
    title: 'Walk by the Spirit',
    scripture: 'Galatians 5:16–25',
    series: null,
    status: 'draft',
    wordCount: 440,
    createdAt: '2026-09-19T08:00:00Z',
    updatedAt: '2026-09-19T23:00:00Z',
    preachedOn: null,
    tags: ['Spirit', 'fruit', 'sanctification'],
    sourcePath: '/home/preacher/sermons/galatians/walk-by-spirit.md',
    fsState: 'local-dirty',
  },
  {
    id: 'sermon-013',
    title: 'The Resurrection and the Life',
    scripture: 'John 11:17–27',
    series: 'Gospel of John',
    status: 'in-progress',
    wordCount: 2100,
    createdAt: '2026-09-10T09:00:00Z',
    updatedAt: '2026-09-17T14:00:00Z',
    preachedOn: null,
    tags: ['resurrection', 'life', 'death'],
    sourcePath: '/home/preacher/sermons/gospel-of-john/resurrection-life.md',
    fsState: 'both-changed',
    hasConflict: true,
  },
  {
    id: 'sermon-014',
    title: 'The Vine and the Branches',
    scripture: 'John 15:1–11',
    series: 'Gospel of John',
    status: 'draft',
    wordCount: 290,
    createdAt: '2026-09-18T10:00:00Z',
    updatedAt: '2026-09-19T08:00:00Z',
    preachedOn: null,
    tags: ['abiding', 'fruit', 'union'],
    sourcePath: '/home/preacher/sermons/gospel-of-john/vine-branches.md',
    fsState: 'missing',
  },
];

// ── Mock sermon documents ─────────────────────────────────────────────────────

const MOCK_DOCUMENT_001: SermonDocument = {
  id: 'sermon-001',
  title: 'The Bread of Life',
  subtitle: 'An Expository Sermon on John 6:35–51',
  scripture: 'John 6:35–51',
  series: 'Gospel of John',
  seriesIndex: 3,
  status: 'in-progress',
  body: `# The Bread of Life

*John 6:35–51 — Expository Sermon — Gospel of John Series, Part 3*

## I. Introduction: The Hunger That Bread Cannot Satisfy

Every person in this room has known physical hunger. We know the gnawing emptiness, the distraction it produces, the singular focus it demands. Jesus draws on this universal experience to address a deeper hunger — the hunger of the soul that no earthly provision can satisfy.

In John 6, the crowd has just witnessed the feeding of five thousand. They are full, satisfied, and enthusiastic. They want to make Jesus king by force (v. 15). But Jesus withdraws. He does not come to be a bread-provider; He comes to be the Bread.

## II. The Claim: "I Am the Bread of Life" (vv. 35–40)

The first of the seven "I AM" declarations in John's Gospel is found here. It is not a modest claim. Jesus says:

> "I am the bread of life; whoever comes to me shall not hunger, and whoever believes in me shall never thirst." — John 6:35 (ESV)

Notice the two verbs: **comes** and **believes**. Coming is the act; believing is the posture. Coming is once; believing is ongoing. The perfect tense in the Greek suggests a settled, permanent state of non-hunger for the one who has come and continues to believe.

The word for bread here is \`G740 ἄρτος (artos)\` — the ordinary word for the loaf of bread that sustained daily life. Jesus is not using an elevated or ceremonial term. He is claiming to be the most basic necessity of human existence, but at the level of the soul.

## III. The Problem: Grumbling Instead of Coming (vv. 41–46)

The crowd grumbles. They know this man's family. He is the carpenter's son from Nazareth. How can he claim to have "come down from heaven"? Their familiarity with his human origins blinds them to his divine nature.

This is the perennial problem of natural religion: we evaluate Jesus by human categories and find him wanting. We need the Father to draw us (v. 44). No one can come to Jesus on their own initiative — this is a sovereign work of grace.

The word \`G1670 ἑλκύω (helkuō)\` — "draws" — is the same word used in John 21:6 for dragging a net full of fish. It is not a gentle suggestion. It is an irresistible, effective drawing.

## IV. The Provision: His Flesh for the Life of the World (vv. 47–51)

Jesus sharpens the metaphor. The bread he gives is his flesh, given for the life of the world. This anticipates the cross. The manna in the wilderness sustained Israel temporarily; the ancestors who ate it still died (v. 49). Jesus offers something qualitatively different — bread that produces eternal life.

Cross-reference: \`Exodus 16:4\` — the original manna narrative. \`Psalm 78:24\` — the psalmist's reflection on it. Both point forward to this moment.

## V. Application: Come, and Keep Coming

The invitation of this passage is not to a one-time event but to a continuous posture of dependence. As we eat physical bread daily, so we must come to Christ daily — through the Word, through prayer, through the gathered community.

Three practical applications for this congregation:

1. **Daily Scripture reading** as the primary means of feeding on Christ
2. **Corporate worship** as the communal table where we eat together
3. **Evangelism** as the invitation to the hungry to come and eat

## VI. Conclusion: The Bread That Satisfies

Augustine wrote: "Thou madest us for Thyself, and our heart is restless, until it repose in Thee." The restlessness Augustine describes is the hunger Jesus addresses. There is a bread that satisfies — not temporarily, not partially, but permanently and completely. That bread is Christ himself.

Come to him. Keep coming. And you shall not hunger.`,
  outline: [
    { id: 'node-1', level: 1, text: 'Introduction: The Hunger That Bread Cannot Satisfy', children: [] },
    {
      id: 'node-2', level: 1, text: 'The Claim: "I Am the Bread of Life" (vv. 35–40)',
      children: [
        { id: 'node-2-1', level: 2, text: 'Two verbs: comes and believes', children: [] },
        { id: 'node-2-2', level: 2, text: 'G740 ἄρτος — ordinary bread of daily life', children: [] },
      ],
    },
    { id: 'node-3', level: 1, text: 'The Problem: Grumbling Instead of Coming (vv. 41–46)', children: [
      { id: 'node-3-1', level: 2, text: 'G1670 ἑλκύω — irresistible drawing', children: [] },
    ]},
    { id: 'node-4', level: 1, text: 'The Provision: His Flesh for the Life of the World (vv. 47–51)', children: [] },
    { id: 'node-5', level: 1, text: 'Application: Come, and Keep Coming', children: [
      { id: 'node-5-1', level: 2, text: 'Daily Scripture reading', children: [] },
      { id: 'node-5-2', level: 2, text: 'Corporate worship', children: [] },
      { id: 'node-5-3', level: 2, text: 'Evangelism', children: [] },
    ]},
    { id: 'node-6', level: 1, text: 'Conclusion: The Bread That Satisfies', children: [] },
  ],
  tags: ['incarnation', 'faith', 'sustenance'],
  createdAt: '2026-09-01T09:00:00Z',
  updatedAt: '2026-09-19T21:45:00Z',
  preachedOn: null,
  version: 14,
  directives: [
    { key: 'translation', value: 'ESV' },
    { key: 'occasion', value: 'Sunday Morning' },
    { key: 'audience', value: 'Congregation' },
    { key: 'series-part', value: '3' },
  ],
  sourcePath: '/home/preacher/sermons/gospel-of-john/bread-of-life.md',
  fsState: 'local-dirty',
  exportHistory: [
    { snapshotId: 'snap-001-v12', format: 'pulpit_manuscript', exportedAt: '2026-09-15T10:30:00Z', outputPath: '/home/preacher/sermons/exports/bread-of-life-v12.pdf', success: true },
  ],
};

// Sermon with conflict state
const MOCK_DOCUMENT_005: SermonDocument = {
  id: 'sermon-005',
  title: 'No Condemnation',
  scripture: 'Romans 8:1–11',
  series: 'Romans: The Gospel Unpacked',
  status: 'preached',
  body: `# No Condemnation

*Romans 8:1–11 — Expository Sermon*

## I. The Declaration: No Condemnation (v. 1)

Paul opens chapter 8 with one of the most sweeping declarations in all of Scripture: "There is therefore now no condemnation for those who are in Christ Jesus." The word \`G2631 κατάκριμα (katakrima)\` — condemnation — refers not merely to a verdict but to the execution of that verdict. Paul is saying that for those in Christ, the sentence has been vacated.

## II. The Basis: The Law of the Spirit (vv. 2–4)

The "law of the Spirit of life" has set us free from the "law of sin and death." This is not an abrogation of the Mosaic law but its fulfillment — what the law could not do (condemn sin in the flesh), God did by sending his Son.

## III. The Application: Mind Set on the Spirit (vv. 5–11)

The practical outworking of this freedom is a mind set on the things of the Spirit rather than the flesh. This is not moral effort but a reorientation of the whole person toward God.`,
  outline: [
    { id: 'n1', level: 1, text: 'The Declaration: No Condemnation (v. 1)', children: [] },
    { id: 'n2', level: 1, text: 'The Basis: The Law of the Spirit (vv. 2–4)', children: [] },
    { id: 'n3', level: 1, text: 'The Application: Mind Set on the Spirit (vv. 5–11)', children: [] },
  ],
  tags: ['condemnation', 'Spirit', 'freedom'],
  createdAt: '2026-07-20T09:00:00Z',
  updatedAt: '2026-09-01T10:00:00Z',
  preachedOn: '2026-09-01',
  version: 8,
  directives: [{ key: 'translation', value: 'ESV' }, { key: 'occasion', value: 'Sunday Morning' }],
  sourcePath: '/home/preacher/sermons/romans/no-condemnation.md',
  fsState: 'disk-changed',
};

// Sermon with unknown directive (codec test)
const MOCK_DOCUMENT_DIRECTIVE: SermonDocument = {
  id: 'sermon-directive-test',
  title: 'Directive Transport Test Sermon',
  scripture: 'John 1:1',
  series: null,
  status: 'draft',
  body: `# Directive Transport Test

This sermon contains known and unknown directives for codec testing.

:::big-idea{text="Christ is the eternal Word who became flesh to dwell among us"}
The central claim of the Johannine prologue is the incarnation of the eternal Logos.
:::

Normal paragraph content between directives.

:::custom-block{foo="bar"}
This is an unknown directive body. It must survive round-trip without modification.
:::

More content after the unknown directive.

:::application{point="1"}
Believe that Jesus is the Christ, the Son of God, and that by believing you may have life in his name.
:::`,
  outline: [],
  tags: ['codec-test'],
  createdAt: '2026-09-19T00:00:00Z',
  updatedAt: '2026-09-19T00:00:00Z',
  preachedOn: null,
  version: 1,
  directives: [{ key: 'translation', value: 'ESV' }],
  sourcePath: '/home/preacher/sermons/test/directive-test.md',
  fsState: 'clean',
};

// ── Static lint fixtures (Rust Track E rule IDs) ─────────────────────────────
// Deterministic preview data conforming to the Rust linter transport contract.
// Canonical rule IDs (from the Rust structural linter):
//   missing-big-idea · orphaned-movement · missing-application ·
//   illustration-fatigue-90d
// The mock NEVER computes rules from document content — Rust does that.

const lint = (
  id: string,
  severity: 'error' | 'warning' | 'info',
  ruleId: string,
  message: string,
  extra: Partial<LintFinding> = {},
): LintFinding => ({ id, severity, ruleId, code: ruleId, message, ...extra });

const LINT_FIXTURES: Record<string, LintFinding[]> = {
  default: [
    lint('fixture-big-idea-1', 'warning', 'missing-big-idea',
      'No Big Idea statement found in the frontmatter or :::big-idea directive.',
      { location: 'frontmatter', suggestedAction: 'State the central proposition as the sermon Big Idea.' }),
    lint('fixture-orphan-1', 'warning', 'orphaned-movement',
      'Movement "The Provision" has no supporting sub-points or resolved warrant.',
      { location: 'outline', movementId: 'node-4', suggestedAction: 'Add sub-points or a warrant statement to the movement.' }),
    lint('fixture-fatigue-1', 'info', 'illustration-fatigue-90d',
      'Illustration "the prodigal son" was used 4 times in the last 90 days.',
      { location: 'body', suggestedAction: 'Replace or supplement with a less-recently-used illustration.' }),
  ],
  'sermon-012': [
    lint('fixture-big-idea-2', 'warning', 'missing-big-idea',
      'No Big Idea statement found in the frontmatter or :::big-idea directive.',
      { location: 'frontmatter', suggestedAction: 'State the central proposition as the sermon Big Idea.' }),
    lint('fixture-application-2', 'warning', 'missing-application',
      'No application section or :::application directive found.',
      { location: 'body', suggestedAction: 'Add concrete application for the congregation.' }),
    lint('fixture-orphan-2', 'warning', 'orphaned-movement',
      'Movement 1 has no supporting sub-points or resolved warrant.',
      { location: 'outline', movementId: 'movement-1', suggestedAction: 'Add sub-points or a warrant statement to the movement.' }),
  ],
};

// ── Static reference fixtures (Track A resolution states) ────────────────────
// Deterministic preview data covering all three Track A resolution states:
// definite · ambiguous (open-ended/ff) · invalid. No parsing happens here —
// the mock never imitates the Rust reference parser.

const REFERENCE_FIXTURES: ReferenceMatch[] = [
  {
    raw: 'John 6:35', book: 'John', chapter: 6, verse: 35, endVerse: null,
    offset: 0, length: 10, osisId: 'John.6.35', resolution: 'definite',
  },
  {
    raw: 'Romans 8:26–27', book: 'Romans', chapter: 8, verse: 26, endVerse: 27,
    offset: 20, length: 15, osisId: 'Rom.8.26-Rom.8.27', resolution: 'definite',
  },
  {
    raw: 'John 15:1ff', book: 'John', chapter: 15, verse: 1, endVerse: null,
    offset: 45, length: 11, resolution: 'ambiguous',
    reason: 'Open-ended reference ("ff") — end verse is ambiguous.',
  },
  {
    raw: 'Maccabees 3:2', book: 'Maccabees', chapter: 3, verse: 2, endVerse: null,
    offset: 66, length: 13, resolution: 'invalid',
    reason: 'Book is not in the canonical book table.',
  },
];

// ── MockSermonBackend ─────────────────────────────────────────────────────────

export class MockSermonBackend implements SermonBackend {
  private sermons: SermonSummary[] = [...MOCK_SERMONS];
  private librarianEnabled = true;
  private settings: AppSettings = {
    libraryPath: '/home/preacher/sermons',
    librarianEnabled: true,
    defaultTranslation: 'ESV',
    autosaveIntervalSeconds: 3,
    editorFontSize: 16,
    editorFont: 'iowan-old-style',
    spellcheck: false,
    focusMode: false,
    exportDefaults: {
      pageSize: 'letter',
      fontSize: 11,
      typographyPreset: 'default',
      includeTitlePage: true,
      includeScriptureReferences: true,
      includeNotes: true,
      includeIllustrations: true,
    },
    keyboardShortcuts: {
      'new-sermon': 'Cmd+N',
      'save': 'Cmd+S',
      'export': 'Cmd+Shift+E',
      'run-lint': 'Cmd+Shift+L',
      // Fix 1: Cmd+P = command palette, Cmd+K = focus archive search
      'command-palette': 'Cmd+P',
      'focus-archive-search': 'Cmd+K',
    },
    developerMode: true,
  };

  // ── List / search ──────────────────────────────────────────────────────────

  async listSermons(): Promise<SermonSummary[]> {
    await delay();
    return [...this.sermons];
  }

  async searchSermons(query: string, _filters?: SearchFilters): Promise<SearchResult[]> {
    await delay(150);
    if (!query.trim()) return [];
    const q = query.toLowerCase();
    return this.sermons
      .filter(
        (s) =>
          s.title.toLowerCase().includes(q) ||
          s.scripture.toLowerCase().includes(q) ||
          (s.series?.toLowerCase().includes(q) ?? false) ||
          s.tags.some((t) => t.toLowerCase().includes(q))
      )
      .map((s) => ({
        id: s.id,
        title: s.title,
        scripture: s.scripture,
        snippet: `…found in ${s.series || 'standalone sermon'} — ${s.wordCount.toLocaleString()} words…`,
        score: 0.85,
        matchedFields: ['title'],
      }));
  }

  // ── CRUD ──────────────────────────────────────────────────────────────────

  async createSermon(request?: CreateSermonRequest): Promise<SermonDocument> {
    await delay();
    const id = `sermon-${String(Date.now()).slice(-6)}`;
    const doc: SermonDocument = {
      id,
      title: request?.title || 'Untitled Sermon',
      scripture: request?.scripture || '',
      series: request?.series || null,
      status: 'draft',
      body: '# Untitled Sermon\n\nBegin writing your sermon here…',
      outline: [],
      tags: [],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      preachedOn: null,
      version: 1,
      directives: [{ key: 'translation', value: 'ESV' }],
      sourcePath: `/home/preacher/sermons/untitled-${id}.md`,
      fsState: 'local-dirty',
    };
    this.sermons.unshift({
      id: doc.id,
      title: doc.title,
      scripture: doc.scripture,
      series: doc.series,
      status: doc.status,
      wordCount: 0,
      createdAt: doc.createdAt,
      updatedAt: doc.updatedAt,
      preachedOn: null,
      tags: [],
      sourcePath: doc.sourcePath,
      fsState: 'local-dirty',
    });
    return doc;
  }

  async loadSermon(id: string): Promise<SermonDocument> {
    await delay();
    if (id === 'sermon-001') return { ...MOCK_DOCUMENT_001 };
    if (id === 'sermon-005') return { ...MOCK_DOCUMENT_005 };
    if (id === 'sermon-directive-test') return { ...MOCK_DOCUMENT_DIRECTIVE };
    const summary = this.sermons.find((s) => s.id === id);
    if (!summary) throw new Error(`Sermon ${id} not found`);
    const isLong = summary.wordCount > 4000;
    const isIncomplete = summary.wordCount < 1000;
    return {
      ...MOCK_DOCUMENT_001,
      id: summary.id,
      title: summary.title,
      subtitle: summary.series ? `Part of "${summary.series}"` : undefined,
      scripture: summary.scripture,
      series: summary.series,
      status: summary.status,
      body: isIncomplete
        ? `# ${summary.title}\n\n*${summary.scripture}*\n\nThis sermon is still in early draft stage. The main argument has not yet been developed.`
        : isLong
        ? `# ${summary.title}\n\n*${summary.scripture}*\n\n## I. Introduction\n\nThis is a longer sermon with extensive development of the text. The exposition covers multiple movements and includes detailed word studies, cross-references, and application points.\n\n## II. Exegesis\n\nThe Greek text of this passage presents several interpretive challenges that require careful attention to the original language and historical context.\n\n## III. Theological Synthesis\n\nThe doctrinal implications of this passage connect to the broader biblical-theological narrative of redemption.\n\n## IV. Application\n\nThree concrete applications for the contemporary congregation emerge from this text.\n\n## V. Conclusion\n\nThe sermon concludes with a call to respond to the text's central demand.`
        : `# ${summary.title}\n\n*${summary.scripture}*\n\n## I. Introduction\n\nOpening the text and establishing the context for this passage.\n\n## II. Exposition\n\nWorking through the primary movements of the text.\n\n## III. Application\n\nBringing the text to bear on the congregation's present situation.\n\n## IV. Conclusion\n\nClosing with a call to respond to the Word.`,
      version: 1,
      directives: [{ key: 'translation', value: 'ESV' }],
      sourcePath: summary.sourcePath,
      fsState: summary.fsState,
    };
  }

  async saveSermon(doc: SermonDocument): Promise<SaveResult> {
    await delay(300);
    const idx = this.sermons.findIndex((s) => s.id === doc.id);
    if (idx !== -1) {
      this.sermons[idx] = {
        ...this.sermons[idx],
        title: doc.title,
        scripture: doc.scripture,
        updatedAt: new Date().toISOString(),
        wordCount: doc.body.replace(/<[^>]+>/g, '').split(/\s+/).filter(Boolean).length,
        fsState: 'clean',
      };
    }
    return { success: true, savedAt: new Date().toISOString(), version: doc.version + 1 };
  }

  async renameSermon(request: RenameSermonRequest): Promise<SermonSummary> {
    await delay(200);
    const idx = this.sermons.findIndex((s) => s.id === request.id);
    if (idx === -1) throw new Error(`Sermon ${request.id} not found`);
    this.sermons[idx] = { ...this.sermons[idx], title: request.newTitle, updatedAt: new Date().toISOString() };
    return this.sermons[idx];
  }

  async duplicateSermon(request: DuplicateSermonRequest): Promise<SermonDocument> {
    await delay(400);
    const original = await this.loadSermon(request.id);
    const newId = `sermon-dup-${String(Date.now()).slice(-6)}`;
    const newDoc: SermonDocument = {
      ...original,
      id: newId,
      title: request.newTitle || `${original.title} (Copy)`,
      status: 'draft',
      version: 1,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      preachedOn: null,
      sourcePath: `/home/preacher/sermons/copy-${newId}.md`,
      fsState: 'local-dirty',
      exportHistory: [],
    };
    this.sermons.unshift({
      id: newDoc.id,
      title: newDoc.title,
      scripture: newDoc.scripture,
      series: newDoc.series,
      status: newDoc.status,
      wordCount: original.body.replace(/<[^>]+>/g, '').split(/\s+/).filter(Boolean).length,
      createdAt: newDoc.createdAt,
      updatedAt: newDoc.updatedAt,
      preachedOn: null,
      tags: [...original.tags],
      sourcePath: newDoc.sourcePath,
      fsState: 'local-dirty',
    });
    return newDoc;
  }

  async archiveSermon(id: string): Promise<void> {
    await delay(200);
    const idx = this.sermons.findIndex((s) => s.id === id);
    if (idx !== -1) this.sermons[idx] = { ...this.sermons[idx], status: 'archived' };
  }

  async deleteSermon(id: string): Promise<void> {
    await delay(300);
    this.sermons = this.sermons.filter((s) => s.id !== id);
  }

  async pinSermon(id: string, pinned: boolean): Promise<void> {
    await delay(100);
    const idx = this.sermons.findIndex((s) => s.id === id);
    if (idx !== -1) this.sermons[idx] = { ...this.sermons[idx], isPinned: pinned };
  }

  // ── Compiler / analysis ───────────────────────────────────────────────────
  // Track I contract: the mock does NOT reimplement Rust lint/reference
  // business rules. Rust (Track A/E) owns parsing and rule evaluation; the
  // mock only returns deterministic STATIC FIXTURE RESULTS that conform to
  // the real Rust transport contracts.

  async parseReferences(_text: string): Promise<ReferenceMatch[]> {
    await delay(100);
    // Static reference fixtures covering all three Track A resolution states.
    // Offsets/lengths are fixture constants — no parsing happens here.
    return REFERENCE_FIXTURES.map((f) => ({ ...f }));
  }

  async lintSermon(doc: SermonDocument): Promise<LintFinding[]> {
    await delay(200);
    // Static lint fixtures keyed by representative scenario. Rule IDs match
    // the Rust linter's canonical vocabulary exactly; the UI never sees
    // mock-invented rules. No rule computation happens in TypeScript.
    return (LINT_FIXTURES[doc.id] ?? LINT_FIXTURES.default).map((f) => ({ ...f }));
  }

  // ── Study rail ────────────────────────────────────────────────────────────

  async getPassage(reference: string): Promise<PassageResult> {
    await delay(300);
    const passages: Record<string, PassageResult> = {
      'John 6:35': {
        reference: 'John 6:35', translation: 'ESV',
        text: 'Jesus said to them, "I am the bread of life; whoever comes to me shall not hunger, and whoever believes in me shall never thirst."',
        verses: [{ verse: 35, text: 'Jesus said to them, "I am the bread of life; whoever comes to me shall not hunger, and whoever believes in me shall never thirst."' }],
        osisRef: 'John.6.35',
      },
      'John 6:44': {
        reference: 'John 6:44', translation: 'ESV',
        text: '"No one can come to me unless the Father who sent me draws him. And I will raise him up on the last day."',
        verses: [{ verse: 44, text: '"No one can come to me unless the Father who sent me draws him. And I will raise him up on the last day."' }],
        osisRef: 'John.6.44',
      },
      'Romans 3:23': {
        reference: 'Romans 3:23', translation: 'ESV',
        text: 'for all have sinned and fall short of the glory of God,',
        verses: [{ verse: 23, text: 'for all have sinned and fall short of the glory of God,' }],
        osisRef: 'Rom.3.23',
      },
      'Romans 8:1': {
        reference: 'Romans 8:1', translation: 'ESV',
        text: 'There is therefore now no condemnation for those who are in Christ Jesus.',
        verses: [{ verse: 1, text: 'There is therefore now no condemnation for those who are in Christ Jesus.' }],
        osisRef: 'Rom.8.1',
      },
      'Isaiah 53:5': {
        reference: 'Isaiah 53:5', translation: 'ESV',
        text: 'But he was pierced for our transgressions; he was crushed for our iniquities; upon him was the chastisement that brought us peace, and with his wounds we are healed.',
        verses: [{ verse: 5, text: 'But he was pierced for our transgressions; he was crushed for our iniquities; upon him was the chastisement that brought us peace, and with his wounds we are healed.' }],
        osisRef: 'Isa.53.5',
      },
    };
    return (
      passages[reference] || {
        reference,
        text: `[Mock passage text for ${reference} — ESV translation would appear here in production]`,
        translation: 'ESV',
        verses: [{ verse: 1, text: `Mock verse text for ${reference}. The actual text would be retrieved from the Rust backend's embedded Scripture database.` }],
        osisRef: reference.replace(/\s+/g, '.').replace(':', '.'),
      }
    );
  }

  async getStrongs(id: string): Promise<StrongsEntry> {
    await delay(200);
    const entries: Record<string, StrongsEntry> = {
      G740: {
        id: 'G740', lemma: 'ἄρτος', transliteration: 'artos',
        definition: 'bread, a loaf of bread; food in general; the bread used in the Lord\'s Supper',
        gloss: 'bread', partOfSpeech: 'noun, masculine', occurrences: 97,
        usageExamples: [
          { reference: 'Matthew 4:3', text: 'command these stones to become loaves of bread (ἄρτοι)' },
          { reference: 'John 6:35', text: 'I am the bread (ἄρτος) of life' },
        ],
        relatedIds: ['G2905', 'G4621'],
      },
      G4102: {
        id: 'G4102', lemma: 'πίστις', transliteration: 'pistis',
        definition: 'faith, belief, trust; the body of Christian belief; faithfulness, reliability',
        gloss: 'faith', partOfSpeech: 'noun, feminine', occurrences: 243,
        usageExamples: [
          { reference: 'Romans 3:22', text: 'the righteousness of God through faith (πίστεως) in Jesus Christ' },
          { reference: 'Hebrews 11:1', text: 'Now faith (πίστις) is the assurance of things hoped for' },
        ],
        relatedIds: ['G4100', 'G4103'],
      },
      G2222: {
        id: 'G2222', lemma: 'ζωή', transliteration: 'zoe',
        definition: 'life, both physical and spiritual; the life of God communicated to believers',
        gloss: 'life', partOfSpeech: 'noun, feminine', occurrences: 135,
        usageExamples: [
          { reference: 'John 1:4', text: 'In him was life (ζωή), and the life was the light of men' },
          { reference: 'John 10:10', text: 'I came that they may have life (ζωήν) and have it abundantly' },
        ],
        relatedIds: ['G2198', 'G5590'],
      },
      G1670: {
        id: 'G1670', lemma: 'ἑλκύω', transliteration: 'helkuō',
        definition: 'to draw, drag; to draw irresistibly; used of drawing a net or sword',
        gloss: 'draw', partOfSpeech: 'verb', occurrences: 8,
        usageExamples: [
          { reference: 'John 6:44', text: 'unless the Father who sent me draws (ἑλκύσῃ) him' },
          { reference: 'John 21:6', text: 'they were not able to haul (ἑλκύσαι) it in' },
        ],
        relatedIds: ['G4951'],
      },
      G2631: {
        id: 'G2631', lemma: 'κατάκριμα', transliteration: 'katakrima',
        definition: 'condemnation, the punishment following sentence; the execution of a judicial verdict',
        gloss: 'condemnation', partOfSpeech: 'noun, neuter', occurrences: 3,
        usageExamples: [
          { reference: 'Romans 8:1', text: 'There is therefore now no condemnation (κατάκριμα)' },
          { reference: 'Romans 5:16', text: 'the judgment following one trespass brought condemnation (κατάκριμα)' },
        ],
        relatedIds: ['G2632', 'G2917'],
      },
    };
    return (
      entries[id] || {
        id, lemma: 'λέξις', transliteration: 'lexis',
        definition: `[Mock definition for Strong's ${id} — actual data from Rust backend's embedded lexicon]`,
        gloss: 'word', partOfSpeech: 'noun', occurrences: 12,
      }
    );
  }

  async getCrossReferences(reference: string): Promise<CrossReference[]> {
    await delay(200);
    const xrefs: Record<string, CrossReference[]> = {
      default: [
        { reference: 'Exodus 16:4', snippet: 'Behold, I am about to rain bread from heaven for you…', relevance: 0.92, category: 'OT Background' },
        { reference: 'Psalm 78:24', snippet: 'he rained down on them manna to eat and gave them the grain of heaven.', relevance: 0.88, category: 'OT Background' },
        { reference: 'John 4:14', snippet: 'whoever drinks of the water that I will give him will never be thirsty again', relevance: 0.85, category: 'Johannine Parallel' },
        { reference: 'Isaiah 55:1–2', snippet: 'Come, everyone who thirsts, come to the waters…', relevance: 0.81, category: 'OT Prophecy' },
        { reference: '1 Corinthians 10:3', snippet: 'all ate the same spiritual food,', relevance: 0.76, category: 'NT Parallel' },
        { reference: 'Deuteronomy 8:3', snippet: 'man does not live by bread alone, but man lives by every word that comes from the mouth of the LORD', relevance: 0.72, category: 'OT Background' },
      ],
    };
    return xrefs[reference] || xrefs['default'];
  }

  async getChainStudy(reference: string): Promise<ChainStudyResult> {
    await delay(250);
    // Deterministic preview fixture — same shape as the Rust engine emits.
    const canonicalStudy: Provenance = {
      class: 'biblical-study', sourceId: 'openbible-xrefs', sourceLabel: 'OpenBible.info Cross References',
      licenseCode: 'CC-BY-4.0', attribution: 'OpenBible.info Cross References (OpenBible.info), CC BY 4.0.',
    };
    const yourArchive: Provenance = { class: 'your-archive', sourceLabel: 'Your archive', sermonId: 'sermon-003' };
    return {
      engineVersion: 'chain-study-1.0',
      seedReference: 'John.6.35',
      canonAvailable: true,
      chains: [{
        id: 'chain-John.6.35',
        name: 'Bread of Life',
        seedReference: 'John.6.35',
        references: [
          { reference: 'John 6:37', distance: 1, weight: 0.36, provenance: canonicalStudy },
          { reference: 'John 6:44', distance: 1, weight: 0.3, provenance: canonicalStudy },
          { reference: 'John 6:47', distance: 2, weight: 0.18, provenance: canonicalStudy },
        ],
        score: 0.84,
        evidence: [
          { kind: 'cross-reference', label: 'Cross-reference (rank 60)', value: 'John 6:35', weight: 0.36, provenance: canonicalStudy },
          { kind: 'sourced-topic', label: 'Bread of Life', value: 'top-bread-of-life', weight: 0.1, provenance: canonicalStudy },
        ],
        sourceTopics: ['Bread of Life'],
      }],
      archiveConnections: [
        { sermonId: 'sermon-003', title: 'The Good Shepherd', primaryPassage: 'John 10:11-John 10:18', matchingReferences: ['John 6:37'], provenance: yourArchive },
      ],
      parameters: { maxSearchDepth: 2, maxNeighborsPerNode: 6, maxChainReferences: 12, maxChains: 5, maxArchiveConnections: 10 },
    };
  }

  async getPreachedOn(reference: string): Promise<PreachedResult[]> {
    await delay(150);
    return [
      { sermonId: 'sermon-003', sermonTitle: 'The Good Shepherd', preachedOn: '2026-09-07', series: 'Gospel of John', wordCount: 3105 },
      { sermonId: 'sermon-009', sermonTitle: 'The Prodigal Son Returns', preachedOn: '2026-01-05', series: null, wordCount: 3890 },
      { sermonId: 'sermon-006', sermonTitle: 'The Suffering Servant', preachedOn: '2026-06-29', series: 'Messianic Prophecies', wordCount: 4450 },
    ];
  }

  // ── Index ─────────────────────────────────────────────────────────────────

  async syncIndex(): Promise<IndexOperationResult> {
    await delay(1200);
    return { success: true, message: 'Index synchronized. 2 documents updated.', documentsIndexed: 2, durationMs: 1180 };
  }

  async rebuildIndex(): Promise<IndexOperationResult> {
    await delay(2400);
    return { success: true, message: 'Index rebuilt from scratch. All 14 documents indexed.', documentsIndexed: 14, durationMs: 2350 };
  }

  async rescanLibrary(): Promise<IndexOperationResult> {
    await delay(1800);
    return { success: true, message: 'Library rescanned. 14 files found, 1 new file detected.', documentsIndexed: 14, durationMs: 1750 };
  }

  async repairIndex(): Promise<IndexOperationResult> {
    await delay(3000);
    return { success: true, message: 'Index repaired. 3 inconsistencies resolved.', documentsIndexed: 14, durationMs: 2980 };
  }

  async cancelIndexOperation(): Promise<void> {
    await delay(100);
  }

  async getIndexStatus(): Promise<IndexStatus> {
    await delay(150);
    return {
      indexedFileCount: 14,
      indexVersion: '2.1.0',
      lastReconciliationTime: '2026-09-19T20:00:00Z',
      lastFullScanTime: '2026-09-18T08:00:00Z',
      status: 'idle',
      pendingFiles: 0,
    };
  }

  async getArchiveStats(): Promise<ArchiveStats> {
    await delay(200);
    return {
      totalSermons: 14,
      totalSeries: 6,
      totalWords: 42138,
      lastPreachedOn: '2026-09-07',
      oldestSermon: '2025-10-15',
      newestSermon: '2026-09-19',
      sermonsByStatus: { draft: 4, 'in-progress': 2, reviewed: 1, preached: 5, archived: 2 },
      sermonsByMonth: [
        { month: 'Oct 25', count: 1 },
        { month: 'Nov 25', count: 1 },
        { month: 'Dec 25', count: 1 },
        { month: 'Jan 26', count: 1 },
        { month: 'Feb 26', count: 0 },
        { month: 'Mar 26', count: 0 },
        { month: 'Apr 26', count: 2 },
        { month: 'May 26', count: 1 },
        { month: 'Jun 26', count: 2 },
        { month: 'Jul 26', count: 1 },
        { month: 'Aug 26', count: 2 },
        { month: 'Sep 26', count: 2 },
      ],
      sermonsByBook: [
        { book: 'John', count: 6 },
        { book: 'Romans', count: 2 },
        { book: 'Isaiah', count: 2 },
        { book: 'Matthew', count: 1 },
        { book: 'Luke', count: 1 },
        { book: 'Galatians', count: 1 },
        { book: 'James', count: 1 },
        { book: 'Ephesians', count: 1 },
        { book: 'Jeremiah', count: 1 },
      ],
      averageWordCount: 3010,
      longestSermon: { id: 'sermon-006', title: 'The Suffering Servant', wordCount: 4450 },
      shortestSermon: { id: 'sermon-012', title: 'Walk by the Spirit', wordCount: 440 },
      sermonLengthDistribution: [
        { bucket: '< 1000', count: 2 },
        { bucket: '1000–2000', count: 1 },
        { bucket: '2000–3000', count: 3 },
        { bucket: '3000–4000', count: 6 },
        { bucket: '4000+', count: 2 },
      ],
      applicationDensity: 0.72,
      unresolvedLintCount: 8,
      recentActivity: [
        { sermonId: 'sermon-001', sermonTitle: 'The Bread of Life', action: 'edited', timestamp: '2026-09-19T21:45:00Z' },
        { sermonId: 'sermon-012', sermonTitle: 'Walk by the Spirit', action: 'created', timestamp: '2026-09-19T08:00:00Z' },
        { sermonId: 'sermon-003', sermonTitle: 'The Good Shepherd', action: 'preached', timestamp: '2026-09-07T11:00:00Z' },
        { sermonId: 'sermon-001', sermonTitle: 'The Bread of Life', action: 'exported', timestamp: '2026-09-15T10:30:00Z' },
        { sermonId: 'sermon-009', sermonTitle: 'The Prodigal Son Returns', action: 'archived', timestamp: '2026-01-06T09:00:00Z' },
      ],
    };
  }

  async getIllustrationFatigue(): Promise<IllustrationFatigueResult[]> {
    await delay(200);
    return [
      { illustration: 'the prodigal son', useCount: 4, lastUsedIn: 'The Prodigal Son Returns', lastUsedOn: '2026-01-05', severity: 'high', sermonIds: ['sermon-009', 'sermon-001', 'sermon-004', 'sermon-007'] },
      { illustration: 'city on a hill', useCount: 3, lastUsedIn: 'Blessed Are the Pure in Heart', lastUsedOn: '2026-09-18', severity: 'medium', sermonIds: ['sermon-008', 'sermon-005', 'sermon-002'] },
      { illustration: 'armor metaphor', useCount: 3, lastUsedIn: 'The Armor of God', lastUsedOn: '2026-06-01', severity: 'medium', sermonIds: ['sermon-007', 'sermon-004', 'sermon-011'] },
      { illustration: 'bread / hunger', useCount: 2, lastUsedIn: 'The Bread of Life', lastUsedOn: '2026-09-19', severity: 'low', sermonIds: ['sermon-001', 'sermon-009'] },
      { illustration: 'shepherd / sheep', useCount: 2, lastUsedIn: 'The Good Shepherd', lastUsedOn: '2026-09-07', severity: 'low', sermonIds: ['sermon-003', 'sermon-006'] },
    ];
  }

  async setLibrarianEnabled(enabled: boolean): Promise<void> {
    await delay(100);
    this.librarianEnabled = enabled;
    this.settings.librarianEnabled = enabled;
  }

  // ── Export ────────────────────────────────────────────────────────────────

  async createExportSnapshot(request: CreateExportSnapshotRequest): Promise<ExportSnapshot> {
    await delay(400);
    const sermon = this.sermons.find((s) => s.id === request.sermonId);
    return {
      snapshotId: `snap-${request.sermonId}-${Date.now().toString(36)}`,
      sermonId: request.sermonId,
      sermonTitle: sermon?.title || 'Unknown Sermon',
      createdAt: new Date().toISOString(),
      revisionHash: `sha256:${Math.random().toString(36).slice(2, 18)}`,
      wordCount: sermon?.wordCount || 0,
      status: 'ready',
    };
  }

  async executeExportJob(request: ExportRequest): Promise<ExportResult> {
    await delay(1800);
    const filename = request.options.outputFilename || `sermon-${request.sermonId}`;
    // Fix 2: Map canonical export formats to file extensions
    const ext: Record<string, string> = {
      pulpit_manuscript: 'pdf',
      church_bulletin: 'pdf',
    };
    const outputPath = `${request.options.outputPath || '/home/preacher/sermons/exports'}/${filename}.${ext[request.format] || 'pdf'}`;
    const modeLabel = request.manuscriptMode ? ` (${request.manuscriptMode})` : '';
    return {
      success: true,
      outputPath,
      message: `Exported successfully as ${request.format}${modeLabel}.`,
      format: request.format,
      snapshotId: request.snapshotId,
      exportedAt: new Date().toISOString(),
      fileSizeBytes: Math.floor(Math.random() * 500000) + 50000,
    };
  }

  async exportSermon(request: ExportRequest): Promise<ExportResult> {
    return this.executeExportJob(request);
  }

  async revealExportedFile(_request: RevealFileRequest): Promise<void> {
    await delay(100);
    // In production: Tauri IPC calls shell.open() on the file's directory
  }

  // ── Conflict / filesystem ─────────────────────────────────────────────────

  async resolveConflict(request: ConflictResolution): Promise<SaveResult> {
    await delay(400);
    const idx = this.sermons.findIndex((s) => s.id === request.sermonId);
    if (idx !== -1) {
      this.sermons[idx] = { ...this.sermons[idx], hasConflict: false, fsState: 'clean' };
    }
    return { success: true, savedAt: new Date().toISOString(), version: 16 };
  }

  async prepareDiff(sermonId: string): Promise<DiffPreparationResult> {
    await delay(300);
    return {
      localLines: [
        'There is therefore now no condemnation for those who are in Christ Jesus.',
        '',
        'The word κατάκριμα (katakrima) refers not merely to a verdict but to the execution of that verdict.',
        'Paul is saying that for those in Christ, the sentence has been vacated permanently.',
        '',
        'This is the foundation of all Christian assurance.',
      ],
      diskLines: [
        'There is therefore now no condemnation for those who are in Christ Jesus.',
        '',
        'The word κατάκριμα (katakrima) refers not merely to a verdict but to the execution of that verdict.',
        'Paul is saying that for those in Christ, the sentence has been vacated.',
        '',
        'This is the foundation of Christian assurance.',
      ],
      hunks: [
        {
          localStart: 3, localCount: 1, diskStart: 3, diskCount: 1,
          lines: [
            { kind: 'removed', text: 'Paul is saying that for those in Christ, the sentence has been vacated.' },
            { kind: 'added', text: 'Paul is saying that for those in Christ, the sentence has been vacated permanently.' },
          ],
        },
        {
          localStart: 5, localCount: 1, diskStart: 5, diskCount: 1,
          lines: [
            { kind: 'removed', text: 'This is the foundation of Christian assurance.' },
            { kind: 'added', text: 'This is the foundation of all Christian assurance.' },
          ],
        },
      ],
    };
  }

  async prepareMerge(sermonId: string): Promise<MergePreparationResult> {
    await delay(400);
    return {
      base: 'Paul is saying that for those in Christ, the sentence has been vacated.',
      local: 'Paul is saying that for those in Christ, the sentence has been vacated permanently.',
      disk: 'Paul is saying that for those in Christ, the condemnation has been fully vacated.',
      conflicts: [
        {
          id: 'conflict-1',
          localLines: ['Paul is saying that for those in Christ, the sentence has been vacated permanently.'],
          diskLines: ['Paul is saying that for those in Christ, the condemnation has been fully vacated.'],
        },
      ],
    };
  }

  async getFilesystemStatus(sermonId: string): Promise<FilesystemReconciliationStatus> {
    await delay(200);
    const sermon = this.sermons.find((s) => s.id === sermonId);
    return {
      sermonId,
      state: sermon?.fsState || 'clean',
      sourcePath: sermon?.sourcePath || `/home/preacher/sermons/${sermonId}.md`,
      localModifiedAt: sermon?.updatedAt,
      diskModifiedAt: sermon?.fsState === 'disk-changed' || sermon?.fsState === 'both-changed'
        ? new Date(Date.now() - 3600000).toISOString()
        : undefined,
    };
  }

  async recoverSermon(sermonId: string): Promise<RecoveryResult> {
    await delay(600);
    const idx = this.sermons.findIndex((s) => s.id === sermonId);
    if (idx !== -1) {
      this.sermons[idx] = { ...this.sermons[idx], fsState: 'clean' };
    }
    return {
      success: true,
      recoveredPath: `/home/preacher/sermons/recovered/${sermonId}.md`,
      message: 'Sermon recovered from backup successfully.',
    };
  }

  async reconnectSermon(request: ReconnectRequest): Promise<SermonSummary> {
    await delay(300);
    const idx = this.sermons.findIndex((s) => s.id === request.sermonId);
    if (idx !== -1) {
      this.sermons[idx] = { ...this.sermons[idx], sourcePath: request.newPath, fsState: 'clean' };
      return this.sermons[idx];
    }
    throw new Error(`Sermon ${request.sermonId} not found`);
  }

  // ── Settings ──────────────────────────────────────────────────────────────

  async loadSettings(): Promise<AppSettings> {
    await delay(150);
    return { ...this.settings };
  }

  async saveSettings(settings: AppSettings): Promise<void> {
    await delay(200);
    this.settings = { ...settings };
  }

  // ── Developer / codec test ────────────────────────────────────────────────
  // Fix 5: Delegate to the shared frontend codec in src/editor/codec/.
  // MockSermonBackend provides test fixtures but does NOT own the codec implementation.

  async testDirectiveCodec(input: string): Promise<CodecRoundTripResult> {
    await delay(100);
    // Use the shared frontend transport codec
    return testRoundTrip(input);
  }

  // ── Sermon Intelligence (Track L presents; fixtures only) ─────────────────

  async getSermonInsights(sermonId: string, _limit = 10): Promise<IntelligenceResult> {
    await delay(180);
    // Deterministic fixture cases. Track J's engine computes these for real;
    // the mock only returns static, evidence-backed results.
    if (sermonId === 'sermon-empty' || sermonId === 'sermon-insufficient') {
      return {
        engineVersion: INTELLIGENCE_ENGINE_VERSION,
        generatedAt: INTELLIGENCE_GENERATED_AT,
        subjectSermonId: sermonId,
        subjectReference: undefined,
        biblicalDataAvailable: true,
        insights: [],
      };
    }
    if (sermonId === 'sermon-no-canon') {
      return {
        engineVersion: INTELLIGENCE_ENGINE_VERSION,
        generatedAt: INTELLIGENCE_GENERATED_AT,
        subjectSermonId: sermonId,
        subjectReference: undefined,
        biblicalDataAvailable: false,
        insights: archiveOnlyInsights(),
      };
    }
    return fullInsights(sermonId, 'John 6:35–51');
  }

  async getRelatedSermons(sermonId: string): Promise<IntelligenceResult> {
    await delay(180);
    const result = fullInsights(sermonId, 'John 6:35–51');
    return { ...result, insights: result.insights.filter((i) => i.kind === 'related-sermon') };
  }

  async getPassageHistory(reference: string): Promise<IntelligenceResult> {
    await delay(180);
    return passageHistoryInsights(reference || 'John 6:35');
  }

  // ── Research packets (Track O) ────────────────────────────────────────────
  // Transport-level mock only: canned in-memory attachments. Rust owns real
  // content-addressing, confinement, extraction, and manifest semantics;
  // the mock preserves the transport shapes so the StudyRail surface is
  // developable in the browser. No lint/reference fixtures are touched here.

  private researchStore = new Map<string, ResearchAttachment[]>();
  private researchPages = new Map<string, ExtractedPage[]>();
  private researchSeq = 0;

  private researchKey(sermonId: string, attachmentId: string): string {
    return `${sermonId}/${attachmentId}`;
  }

  async attachResearchFile(
    request: AttachResearchFileRequest,
  ): Promise<ResearchAttachment> {
    await delay(120);
    // Mock identity: content addressing is Rust's job; here a per-file counter.
    this.researchSeq += 1;
    const id = `mock-att-${String(this.researchSeq).padStart(3, '0')}`;
    const filename = request.sourcePath.split(/[\\/]/).pop() ?? 'attachment.pdf';
    const now = new Date().toISOString();
    const attachment: ResearchAttachment = {
      id,
      originalFilename: filename,
      storedFilename: `mock-${id}.pdf`,
      title: request.title,
      author: request.author,
      source: request.source,
      dateAdded: now,
      mimeType: 'application/pdf',
      byteSize: 0,
      checksum: `mock-checksum-${this.researchSeq}`,
      extraction: 'ok',
      importProvenance: { importedFrom: request.sourcePath, importedAt: now },
      provenanceClass: 'research-packet',
    };
    const list = this.researchStore.get(request.sermonId) ?? [];
    // Idempotent dedup mirrors the Rust contract: same source path returns
    // the existing attachment instead of duplicating it.
    const existing = list.find((a) => a.importProvenance.importedFrom === request.sourcePath);
    if (existing) return existing;
    list.push(attachment);
    this.researchStore.set(request.sermonId, list);
    this.researchPages.set(this.researchKey(request.sermonId, id), [
      { page: 1, text: `Mock extraction for ${filename}.\n\nPage 1 text.` },
    ]);
    return attachment;
  }

  async listResearchAttachments(sermonId: string): Promise<ResearchAttachment[]> {
    await delay(60);
    return this.researchStore.get(sermonId) ?? [];
  }

  private requireResearchAttachment(
    sermonId: string,
    attachmentId: string,
  ): ResearchAttachment {
    const att = (this.researchStore.get(sermonId) ?? []).find((a) => a.id === attachmentId);
    if (!att) throw new Error(`attachment not found: ${attachmentId}`);
    return att;
  }

  async getResearchAttachment(
    sermonId: string,
    attachmentId: string,
  ): Promise<ResearchAttachment> {
    await delay(60);
    return this.requireResearchAttachment(sermonId, attachmentId);
  }

  async getExtractedPages(
    sermonId: string,
    attachmentId: string,
  ): Promise<ExtractedPage[]> {
    await delay(60);
    this.requireResearchAttachment(sermonId, attachmentId);
    return this.researchPages.get(this.researchKey(sermonId, attachmentId)) ?? [];
  }

  async updateResearchMetadata(
    sermonId: string,
    attachmentId: string,
    patch: UpdateResearchMetadataRequest,
  ): Promise<ResearchAttachment> {
    await delay(60);
    const att = this.requireResearchAttachment(sermonId, attachmentId);
    const updated: ResearchAttachment = { ...att, ...patch };
    const list = (this.researchStore.get(sermonId) ?? []).map((a) =>
      a.id === attachmentId ? updated : a,
    );
    this.researchStore.set(sermonId, list);
    return updated;
  }

  async removeResearchAttachment(sermonId: string, attachmentId: string): Promise<void> {
    await delay(60);
    this.requireResearchAttachment(sermonId, attachmentId);
    const list = (this.researchStore.get(sermonId) ?? []).filter(
      (a) => a.id !== attachmentId,
    );
    this.researchStore.set(sermonId, list);
    this.researchPages.delete(this.researchKey(sermonId, attachmentId));
  }

  async openResearchFile(
    sermonId: string,
    attachmentId: string,
  ): Promise<OpenResearchFileResult> {
    await delay(60);
    const att = this.requireResearchAttachment(sermonId, attachmentId);
    return { absolutePath: att.importProvenance.importedFrom };
  }
}
