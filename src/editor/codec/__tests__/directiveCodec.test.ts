/**
 * Directive Transport Codec Tests
 *
 * Covers:
 * - movement directive
 * - illustration directive
 * - application directive
 * - exegetical-notes directive
 * - unknown directive
 * - mixed known + unknown directives
 * - unknown attributes
 * - nested ordinary Markdown around directives
 * - round-trip preservation (PASS/FAIL)
 * - byte-equality for unknown directives (rawSource verbatim)
 */

import { parseDirectives, serializeDirectives, testRoundTrip } from '../directiveCodec';
import { describe, it, expect } from '@jest/globals';

// ── Helpers ───────────────────────────────────────────────────────────────────

function roundTrip(input: string): string {
  return serializeDirectives(parseDirectives(input));
}

// ── Individual known directives ───────────────────────────────────────────────

describe('movement directive', () => {
  const input = `:::movement{title="The Eternal Word" index="1"}
In the beginning was the Word, and the Word was with God, and the Word was God.
:::`;

  it('parses as known', () => {
    const [d] = parseDirectives(input);
    expect(d.kind).toBe('known');
    expect(d.name).toBe('movement');
  });

  it('preserves title and index attributes', () => {
    const [d] = parseDirectives(input);
    expect(d.attributes.title).toBe('The Eternal Word');
    expect(d.attributes.index).toBe('1');
  });

  it('preserves body', () => {
    const [d] = parseDirectives(input);
    expect(d.body).toContain('In the beginning was the Word');
  });

  it('round-trips without loss', () => {
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
  });
});

describe('illustration directive', () => {
  const input = `:::illustration{title="Augustine's Restless Heart" source="Confessions"}
Augustine wrote: "Thou madest us for Thyself, and our heart is restless, until it repose in Thee."
:::`;

  it('parses as known', () => {
    const [d] = parseDirectives(input);
    expect(d.kind).toBe('known');
    expect(d.name).toBe('illustration');
  });

  it('preserves title and source attributes', () => {
    const [d] = parseDirectives(input);
    expect(d.attributes.title).toBe("Augustine's Restless Heart");
    expect(d.attributes.source).toBe('Confessions');
  });

  it('round-trips without loss', () => {
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
  });
});

describe('application directive', () => {
  const input = `:::application{point="1"}
Trust in Christ alone for your standing before God — not in your moral performance.
:::`;

  it('parses as known', () => {
    const [d] = parseDirectives(input);
    expect(d.kind).toBe('known');
    expect(d.name).toBe('application');
  });

  it('preserves point attribute', () => {
    const [d] = parseDirectives(input);
    expect(d.attributes.point).toBe('1');
  });

  it('round-trips without loss', () => {
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
  });
});

describe('exegetical-notes directive (known — canonical four)', () => {
  // exegetical-notes IS in the KNOWN_DIRECTIVES set — it is one of the canonical four.
  const input = `:::exegetical-notes{passage="John 1:1" lang="gk"}
ἐν ἀρχῇ ἦν ὁ λόγος — the anarthrous predicate nominative indicates qualitative nature.
:::`;

  it('parses as known (canonical four)', () => {
    const [d] = parseDirectives(input);
    expect(d.kind).toBe('known');
    expect(d.name).toBe('exegetical-notes');
  });

  it('preserves passage and lang attributes verbatim', () => {
    const [d] = parseDirectives(input);
    expect(d.attributes.passage).toBe('John 1:1');
    expect(d.attributes.lang).toBe('gk');
  });

  it('preserves body verbatim', () => {
    const [d] = parseDirectives(input);
    expect(d.body).toContain('ἐν ἀρχῇ ἦν ὁ λόγος');
  });

  it('round-trips without loss', () => {
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
  });
});

// ── Unknown directive ─────────────────────────────────────────────────────────

describe('unknown directive', () => {
  const input = `:::custom-block{foo="bar"}
This is an unknown directive body. It must survive round-trip without modification.
:::`;

  it('parses as unknown', () => {
    const [d] = parseDirectives(input);
    expect(d.kind).toBe('unknown');
    expect(d.name).toBe('custom-block');
  });

  it('preserves foo attribute', () => {
    const [d] = parseDirectives(input);
    expect(d.attributes.foo).toBe('bar');
  });

  it('preserves body verbatim', () => {
    const [d] = parseDirectives(input);
    expect(d.body).toContain('unknown directive body');
  });

  it('round-trips: PASS', () => {
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
  });

  it('serialized output is byte-for-byte equal to rawSource (unknown directives use rawSource)', () => {
    const [d] = parseDirectives(input);
    // Unknown directives must be emitted verbatim from rawSource
    expect(d.rawSource).toBe(input);
    const serialized = roundTrip(input);
    expect(serialized).toBe(d.rawSource);
  });

  it('serialized output contains original header', () => {
    const serialized = roundTrip(input);
    expect(serialized).toContain(':::custom-block{foo="bar"}');
  });
});

// ── Unknown attributes on a known directive ───────────────────────────────────

describe('unknown attributes on known directive', () => {
  // The codec must not strip attributes it does not recognize.
  // Using 'movement' (a canonical known directive) with extra attributes.
  const input = `:::movement{title="Christ is Lord" version="2"}
The central claim of the passage.
:::`;

  it('parses as known', () => {
    const [d] = parseDirectives(input);
    expect(d.kind).toBe('known');
  });

  it('preserves all attributes including extra ones', () => {
    const [d] = parseDirectives(input);
    expect(d.attributes.title).toBe('Christ is Lord');
    expect(d.attributes.version).toBe('2');
  });

  it('round-trips without loss', () => {
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
  });
});

// ── Mixed known + unknown directives ─────────────────────────────────────────

describe('mixed known + unknown directives', () => {
  // Uses only canonical known directives (movement, illustration, application, exegetical-notes)
  // and clearly unknown ones (custom-block, unknown-directive, big-idea, note).
  const input = `:::movement{title="Justification by Faith"}
The central thesis of Romans 3:21–31.
:::

:::custom-block{foo="bar"}
This is an unknown directive body. It must survive round-trip without modification.
:::

:::application{point="1"}
Trust in Christ alone for your standing before God.
:::

:::unknown-directive{x="1" y="2"}
Another unknown directive with multiple attributes.
:::

:::illustration{title="Calvin on Faith"}
Faith alone justifies, but the faith that justifies is never alone.
:::`;

  it('parses 5 directives', () => {
    const parsed = parseDirectives(input);
    expect(parsed).toHaveLength(5);
  });

  it('correctly identifies known vs unknown', () => {
    const parsed = parseDirectives(input);
    expect(parsed[0].kind).toBe('known');   // movement
    expect(parsed[1].kind).toBe('unknown'); // custom-block
    expect(parsed[2].kind).toBe('known');   // application
    expect(parsed[3].kind).toBe('unknown'); // unknown-directive
    expect(parsed[4].kind).toBe('known');   // illustration
  });

  it('preserves unknown directive attributes', () => {
    const parsed = parseDirectives(input);
    expect(parsed[1].attributes.foo).toBe('bar');
    expect(parsed[3].attributes.x).toBe('1');
    expect(parsed[3].attributes.y).toBe('2');
  });

  it('unknown directives emit rawSource byte-for-byte', () => {
    const parsed = parseDirectives(input);
    const serialized = serializeDirectives(parsed);
    // Each unknown directive's rawSource must appear verbatim in the output
    for (const d of parsed.filter((p) => p.kind === 'unknown')) {
      expect(serialized).toContain(d.rawSource);
    }
  });

  it('round-trips: PASS', () => {
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
  });

  it('serialized output preserves ordering', () => {
    const parsed = parseDirectives(input);
    const serialized = serializeDirectives(parsed);
    const movementPos = serialized.indexOf(':::movement');
    const customPos = serialized.indexOf(':::custom-block');
    const appPos = serialized.indexOf(':::application');
    const unknownPos = serialized.indexOf(':::unknown-directive');
    const illustrationPos = serialized.indexOf(':::illustration');
    expect(movementPos).toBeLessThan(customPos);
    expect(customPos).toBeLessThan(appPos);
    expect(appPos).toBeLessThan(unknownPos);
    expect(unknownPos).toBeLessThan(illustrationPos);
  });
});

// ── Nested ordinary Markdown around directives ────────────────────────────────

describe('ordinary Markdown around directives', () => {
  // The codec only parses directives; surrounding Markdown is not parsed.
  // Directives embedded in a larger Markdown document must still be found.
  const input = `# Sermon Title

Some introductory prose before the first directive.

:::movement{title="The Word became flesh"}
The incarnation is the hinge of redemptive history.
:::

More prose between directives. This is ordinary Markdown.

- A bullet point
- Another bullet point

:::illustration{title="Bread of Life"}
Jesus said: I am the bread of life.
:::

Concluding prose after the last directive.`;

  it('finds both directives in surrounding Markdown', () => {
    const parsed = parseDirectives(input);
    expect(parsed).toHaveLength(2);
    expect(parsed[0].name).toBe('movement');
    expect(parsed[1].name).toBe('illustration');
  });

  it('round-trips directives without loss', () => {
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
  });
});

// ── Round-trip preservation ───────────────────────────────────────────────────

describe('round-trip preservation', () => {
  it('empty input produces PASS with no directives', () => {
    const result = testRoundTrip('');
    expect(result.pass).toBe(true);
    expect(result.parsed).toHaveLength(0);
  });

  it('input with no directives produces PASS', () => {
    const result = testRoundTrip('# Just a heading\n\nSome prose without any directives.');
    expect(result.pass).toBe(true);
    expect(result.parsed).toHaveLength(0);
  });

  it('unknown directive with no attributes round-trips byte-for-byte', () => {
    const input = `:::bare-unknown\nBody content here.\n:::`;
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
    const [d] = result.parsed;
    expect(d.kind).toBe('unknown');
    expect(d.name).toBe('bare-unknown');
    // Byte-for-byte: serialized single directive equals rawSource
    expect(result.serialized).toBe(d.rawSource);
  });

  it('multiple unknown directives all survive byte-for-byte', () => {
    const input = `:::alpha{a="1"}\nAlpha body.\n:::\n\n:::beta{b="2"}\nBeta body.\n:::`;
    const result = testRoundTrip(input);
    expect(result.pass).toBe(true);
    expect(result.parsed).toHaveLength(2);
    expect(result.parsed[0].kind).toBe('unknown');
    expect(result.parsed[1].kind).toBe('unknown');
    // Each unknown directive's rawSource must appear verbatim in serialized output
    expect(result.serialized).toContain(result.parsed[0].rawSource);
    expect(result.serialized).toContain(result.parsed[1].rawSource);
  });

  it('serialized output contains all known directive headers', () => {
    const input = `:::movement{title="M1"}\nBody.\n:::\n\n:::application{point="2"}\nApp.\n:::`;
    const { serialized } = testRoundTrip(input);
    expect(serialized).toContain(':::movement{title="M1"}');
    expect(serialized).toContain(':::application{point="2"}');
  });

  it('testRoundTrip returns structured result', () => {
    const input = `:::movement{title="Test"}\nBody.\n:::`;
    const result = testRoundTrip(input);
    expect(result).toHaveProperty('pass');
    expect(result).toHaveProperty('input');
    expect(result).toHaveProperty('parsed');
    expect(result).toHaveProperty('serialized');
    expect(result.input).toBe(input);
  });

  it('unknown directive: serialized equals rawSource exactly (byte-equality)', () => {
    const input = `:::my-custom{key="value" other="data"}\nSome body text here.\n:::`;
    const [d] = parseDirectives(input);
    expect(d.kind).toBe('unknown');
    // The serialized form of a single unknown directive must be byte-equal to rawSource
    const serialized = serializeDirectives([d]);
    expect(serialized).toBe(d.rawSource);
  });
});
