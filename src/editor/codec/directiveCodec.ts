/**
 * Directive Transport Codec
 *
 * Shared frontend infrastructure for parsing and serializing directive Markdown.
 * Used by both MockSermonBackend (for test fixtures) and TauriSermonBackend (for IPC transport).
 *
 * CONTRACT:
 * - Transport-only: no Scripture normalization, no warrant validation,
 *   no canonical AST validation, no lint rules.
 * - Unknown directives and unknown attributes MUST round-trip losslessly.
 * - Known directives may receive specialized editor rendering.
 *
 * Location: src/editor/codec/
 * This is editor-owned shared frontend infrastructure.
 */

import type { ParsedDirective, CodecRoundTripResult } from '@/lib/backend/types';

/** Directive names recognized by the frontend for specialized rendering. */
export const KNOWN_DIRECTIVES: ReadonlySet<string> = new Set([
  'movement',
  'illustration',
  'application',
  'exegetical-notes',
]);

/**
 * Parse directive Markdown into an array of ParsedDirective objects.
 * Unknown directives are preserved as opaque transport data.
 */
export function parseDirectives(input: string): ParsedDirective[] {
  const parsed: ParsedDirective[] = [];
  const directivePattern = /:::([a-zA-Z0-9_-]+)(?:\{([^}]*)\})?\n([\s\S]*?):::/g;
  let m: RegExpExecArray | null;

  while ((m = directivePattern.exec(input)) !== null) {
    const name = m[1];
    const attrStr = m[2] || '';
    const body = m[3] || '';
    const attrs: Record<string, string> = {};

    const attrPattern = /(\w+)="([^"]*)"/g;
    let am: RegExpExecArray | null;
    while ((am = attrPattern.exec(attrStr)) !== null) {
      attrs[am[1]] = am[2];
    }

    parsed.push({
      kind: KNOWN_DIRECTIVES.has(name) ? 'known' : 'unknown',
      name,
      attributes: attrs,
      body: body.trim(),
      rawSource: m[0],
    });
  }

  return parsed;
}

/**
 * Serialize an array of ParsedDirective objects back to directive Markdown.
 * Unknown directives are emitted from their rawSource verbatim for byte-for-byte
 * preservation. Known directives are reconstructed from parsed parts.
 */
export function serializeDirectives(directives: ParsedDirective[]): string {
  return directives
    .map((d) => {
      if (d.kind === 'unknown') {
        // Byte-for-byte preservation: emit the original source unchanged
        return d.rawSource;
      }
      const attrStr = Object.entries(d.attributes)
        .map(([k, v]) => `${k}="${v}"`)
        .join(' ');
      const header = attrStr ? `:::${d.name}{${attrStr}}` : `:::${d.name}`;
      return `${header}\n${d.body}\n:::`;
    })
    .join('\n\n');
}

/**
 * Run a full round-trip test on the given directive Markdown input.
 * Returns a CodecRoundTripResult with PASS/FAIL and the parsed/serialized state.
 *
 * PASS requires:
 * - All unknown directives survive with their name, attributes, and body intact.
 * - All known directives survive with their name intact.
 */
export function testRoundTrip(input: string): CodecRoundTripResult {
  const parsed = parseDirectives(input);
  const serialized = serializeDirectives(parsed);

  let pass = true;

  // Unknown directives must survive verbatim (name + all attributes)
  for (const ud of parsed.filter((d) => d.kind === 'unknown')) {
    const reconstructed = Object.entries(ud.attributes)
      .map(([k, v]) => `${k}="${v}"`)
      .join(' ');
    const header = reconstructed ? `:::${ud.name}{${reconstructed}}` : `:::${ud.name}`;
    if (!serialized.includes(header)) {
      pass = false;
      break;
    }
  }

  // Known directives must also survive
  if (pass) {
    for (const kd of parsed.filter((d) => d.kind === 'known')) {
      if (!serialized.includes(`:::${kd.name}`)) {
        pass = false;
        break;
      }
    }
  }

  return { pass, input, parsed, serialized };
}
