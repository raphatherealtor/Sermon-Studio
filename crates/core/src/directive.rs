//! Canonical directive parsing and serialization for the Rust sermon model.
//!
//! Track B owns this module. The canonical known directive set is EXACTLY:
//!
//! - `movement`
//! - `illustration`
//! - `application`
//! - `exegetical-notes`
//!
//! Everything else is an *unknown* directive and is archival data: it carries
//! its original raw source representation and is emitted byte-for-byte on
//! serialization. Unknown attributes on known directives are likewise
//! preserved (order and duplicates retained) so they survive round-trip.
//!
//! The frontend transport codec
//! (`src/editor/codec/directiveCodec.ts`) is a parity source for the
//! invariants tested here; Rust is authoritative for canonical behavior.

use serde::{Deserialize, Serialize};

/// The canonical known directive set (exactly these four).
pub const KNOWN_DIRECTIVES: [&str; 4] = [
    "movement",
    "illustration",
    "application",
    "exegetical-notes",
];

/// True when `name` is one of the canonical known directives.
pub fn is_known_directive(name: &str) -> bool {
    KNOWN_DIRECTIVES.contains(&name)
}

/// Byte-offset span into the original Markdown source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Offset of the first byte (inclusive).
    pub start: usize,
    /// Offset one past the last byte (exclusive).
    pub end: usize,
}

impl SourceSpan {
    pub fn new(start: usize, end: usize) -> Self {
        SourceSpan { start, end }
    }

    /// Length of the span in bytes.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// A single directive attribute. Order and duplicates are preserved so that
/// unknown attributes survive a parse → serialize round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectiveAttribute {
    pub key: String,
    pub value: String,
    /// True when the source had a bare key with no `="value"` pair. Bare keys
    /// are re-emitted bare (not as `key=""`) to stay close to the source.
    #[serde(default)]
    pub bare: bool,
}

impl DirectiveAttribute {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        DirectiveAttribute {
            key: key.into(),
            value: value.into(),
            bare: false,
        }
    }

    pub fn bare(key: impl Into<String>) -> Self {
        DirectiveAttribute {
            key: key.into(),
            value: String::new(),
            bare: true,
        }
    }

    /// Render as `key="value"` (or a bare key) as it appears inside `{...}`.
    pub fn to_source(&self) -> String {
        if self.bare {
            self.key.clone()
        } else {
            format!("{}=\"{}\"", self.key, self.value)
        }
    }
}

/// A parsed directive block.
///
/// For unknown directives, `raw_source` is the authoritative representation:
/// it is emitted verbatim on serialization and must never be normalized,
/// rewritten, or "fixed".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Directive {
    /// Directive name as written after the opening `:::`, e.g. `movement`.
    pub name: String,
    /// Whether this is one of the four canonical known directives.
    pub known: bool,
    /// Attributes in source order. Unknown attributes are preserved here.
    pub attributes: Vec<DirectiveAttribute>,
    /// Block body with surrounding blank lines trimmed (semantic view).
    /// The raw source (including any odd whitespace) remains available via
    /// `raw_source`.
    pub body: String,
    /// Byte-exact source of the whole block, from the opening fence line
    /// through the closing `:::` line (including newlines as written).
    pub raw_source: String,
    /// Span of `raw_source` within the parsed input.
    pub span: SourceSpan,
}

impl Directive {
    /// First value for `key`, if present.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.key == key)
            .map(|a| a.value.as_str())
    }

    /// Remove and return the first attribute matching any of `keys`.
    /// Remaining attributes are the "unknown" set that must survive
    /// round-trip.
    pub fn take_attr(&mut self, keys: &[&str]) -> Option<String> {
        let pos = self
            .attributes
            .iter()
            .position(|a| keys.contains(&a.key.as_str()))?;
        Some(self.attributes.remove(pos).value)
    }

    /// Serialize to directive Markdown. Unknown directives emit their
    /// `raw_source` verbatim (byte-for-byte archival preservation); known
    /// directives are reconstructed from their parsed parts with attributes
    /// in original order.
    pub fn to_markdown(&self) -> String {
        if !self.known {
            return self.raw_source.clone();
        }
        let mut out = String::new();
        out.push_str(":::");
        out.push_str(&self.name);
        if !self.attributes.is_empty() {
            let attrs: Vec<String> = self.attributes.iter().map(|a| a.to_source()).collect();
            out.push('{');
            out.push_str(&attrs.join(" "));
            out.push('}');
        }
        out.push('\n');
        out.push_str(self.body.trim());
        out.push_str("\n:::");
        out
    }
}
/// An opening fence line: `:::name` or `:::name{attrs}`, no leading
/// whitespace, nothing after the optional `{...}` but spaces.
struct OpenFence<'a> {
    name: &'a str,
    attrs: &'a str,
}

fn parse_open_fence(line: &str) -> Option<OpenFence<'_>> {
    let rest = line.strip_prefix(":::")?;
    // The fence name must not itself start with another ':' (that would be a
    // closing fence or a longer run) — closing fences are handled separately.
    let mut end = 0usize;
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            end += c.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let name = &rest[..end];
    let after = rest[end..].trim();
    if after.is_empty() {
        return Some(OpenFence { name, attrs: "" });
    }
    let inner = after.strip_prefix('{')?.strip_suffix('}')?;
    // Reject stray braces inside the attribute region.
    if inner.contains('{') || inner.contains('}') {
        return None;
    }
    Some(OpenFence { name, attrs: inner })
}

fn is_close_fence(line: &str) -> bool {
    line.trim() == ":::"
}

/// Parse `{...}` attribute content into ordered attributes. Supports
/// `key="value"` pairs (double quotes) and bare keys. Unrecognized leftovers
/// are skipped; unknown directives remain byte-exact via `raw_source` anyway,
/// and known directives carry their unknown attributes through `attributes`.
fn parse_attributes(attr_src: &str) -> Vec<DirectiveAttribute> {
    let mut attrs = Vec::new();
    let bytes = attr_src.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // Skip whitespace and stray punctuation between tokens.
        let c = bytes[i] as char;
        if !(c.is_ascii_alphanumeric() || c == '_') {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c.is_ascii_alphanumeric() || c == '_' {
                i += 1;
            } else {
                break;
            }
        }
        let key = &attr_src[start..i];
        // Optional = "value"
        let mut j = i;
        while j < bytes.len() && (bytes[j] as char).is_whitespace() {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b'=' {
            j += 1;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'"' {
                j += 1;
                let vstart = j;
                while j < bytes.len() && bytes[j] != b'"' {
                    j += 1;
                }
                let value = &attr_src[vstart..j.min(bytes.len())];
                attrs.push(DirectiveAttribute::new(key, value));
                i = if j < bytes.len() { j + 1 } else { j };
                continue;
            }
            // Unquoted value (canonical directives allow `order=1`): read
            // until whitespace or end of the attribute region.
            let vstart = j;
            while j < bytes.len() && !(bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j > vstart {
                attrs.push(DirectiveAttribute::new(key, &attr_src[vstart..j]));
                i = j;
                continue;
            }
            // `=` with no value at all: treat as bare key, leave i at the key
            // end so the '=' is skipped as stray punctuation next round.
            attrs.push(DirectiveAttribute::bare(key));
            continue;
        }
        attrs.push(DirectiveAttribute::bare(key));
        i = j;
    }
    attrs
}

/// Parse all directive blocks in `input`. Surrounding Markdown is not
/// parsed here; spans allow the caller to split the document into blocks.
///
/// An unterminated opening fence (no closing `:::` line) is treated as
/// ordinary Markdown, never as an error, so partially-typed text cannot
/// corrupt a document on load/save.
pub fn parse_directives(input: &str) -> Vec<Directive> {
    let mut out = Vec::new();
    let mut line_start = 0usize;
    let lines: Vec<&str> = input.split_inclusive('\n').collect();
    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx];
        let line_end = line_start + line.len();
        let content = line.trim_end_matches(['\n', '\r']);

        if let Some(fence) = parse_open_fence(content) {
            // Look ahead for the closing fence.
            let body_start = line_end;
            let mut scan = line_end;
            let mut close: Option<(usize, usize, usize)> = None; // (content_start, content_end, line_end)
            let mut j = idx + 1;
            while j < lines.len() {
                let l2 = lines[j];
                let t2 = l2.trim_end_matches(['\n', '\r']);
                if is_close_fence(t2) {
                    let content_end = scan + t2.len();
                    close = Some((scan, content_end, scan + l2.len()));
                    break;
                }
                scan += l2.len();
                j += 1;
            }
            if let Some((close_start, close_end, line_full_end)) = close {
                // raw_source ends at the closing fence content (without the
                // line's trailing newline), matching transport-codex parity:
                // a standalone serialized directive is byte-equal to
                // raw_source. Newlines following the block belong to the
                // surrounding Markdown gaps.
                let raw_source = input[line_start..close_end].to_string();
                let body = input[body_start..close_start]
                    .trim_end_matches(['\n', '\r'])
                    .to_string();
                let name = fence.name.to_string();
                let known = is_known_directive(&name);
                out.push(Directive {
                    name,
                    known,
                    attributes: parse_attributes(fence.attrs),
                    body,
                    raw_source,
                    span: SourceSpan::new(line_start, close_end),
                });
                line_start = line_full_end;
                idx = j + 1;
                continue;
            }
            // Unterminated: leave as ordinary Markdown, continue scanning.
        }

        line_start = line_end;
        idx += 1;
    }
    out
}

/// Serialize directives back to directive Markdown, joined with a blank
/// line. Unknown directives are emitted from `raw_source` verbatim.
pub fn serialize_directives(directives: &[Directive]) -> String {
    directives
        .iter()
        .map(|d| d.to_markdown())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_is_known_with_attrs_and_body() {
        let input = ":::movement{title=\"The Eternal Word\" index=\"1\"}\nIn the beginning was the Word, and the Word was with God, and the Word was God.\n:::";
        let parsed = parse_directives(input);
        assert_eq!(parsed.len(), 1);
        let d = &parsed[0];
        assert!(d.known);
        assert_eq!(d.name, "movement");
        assert_eq!(d.attr("title"), Some("The Eternal Word"));
        assert_eq!(d.attr("index"), Some("1"));
        assert!(d.body.contains("In the beginning was the Word"));
    }

    #[test]
    fn illustration_is_known() {
        let input = ":::illustration{title=\"Augustine's Restless Heart\" source=\"Confessions\"}\nAugustine wrote: \"Thou madest us for Thyself.\"\n:::";
        let d = &parse_directives(input)[0];
        assert!(d.known);
        assert_eq!(d.name, "illustration");
        assert_eq!(d.attr("title"), Some("Augustine's Restless Heart"));
        assert_eq!(d.attr("source"), Some("Confessions"));
    }

    #[test]
    fn application_is_known() {
        let input = ":::application{point=\"1\"}\nTrust in Christ alone for your standing before God.\n:::";
        let d = &parse_directives(input)[0];
        assert!(d.known);
        assert_eq!(d.name, "application");
        assert_eq!(d.attr("point"), Some("1"));
    }

    #[test]
    fn exegetical_notes_is_known_and_body_verbatim() {
        let input = ":::exegetical-notes{passage=\"John 1:1\" lang=\"gk\"}\nἐν ἀρχῇ ἦν ὁ λόγος — the anarthrous predicate nominative indicates qualitative nature.\n:::";
        let d = &parse_directives(input)[0];
        assert!(d.known);
        assert_eq!(d.name, "exegetical-notes");
        assert_eq!(d.attr("passage"), Some("John 1:1"));
        assert_eq!(d.attr("lang"), Some("gk"));
        assert!(d.body.contains("ἐν ἀρχῇ ἦν ὁ λόγος"));
    }

    #[test]
    fn unknown_directive_keeps_raw_source_byte_exact() {
        let input = ":::custom-block{foo=\"bar\"}\nThis is an unknown directive body. It must survive round-trip without modification.\n:::";
        let d = &parse_directives(input)[0];
        assert!(!d.known);
        assert_eq!(d.name, "custom-block");
        assert_eq!(d.attr("foo"), Some("bar"));
        assert_eq!(d.raw_source, input);
        assert_eq!(d.to_markdown(), input);
        assert_eq!(serialize_directives(&[d.clone()]), input);
    }

    #[test]
    fn unknown_attributes_on_known_directive_are_preserved() {
        let input = ":::movement{title=\"Christ is Lord\" version=\"2\"}\nThe central claim of the passage.\n:::";
        let d = &parse_directives(input)[0];
        assert!(d.known);
        assert_eq!(d.attr("title"), Some("Christ is Lord"));
        assert_eq!(d.attr("version"), Some("2"));
        assert!(d.to_markdown().contains("version=\"2\""));
    }

    #[test]
    fn mixed_known_and_unknown_directives() {
        let input = ":::movement{title=\"Justification by Faith\"}\nThe central thesis of Romans 3:21–31.\n:::\n\n:::custom-block{foo=\"bar\"}\nUnknown body.\n:::\n\n:::application{point=\"1\"}\nTrust in Christ alone.\n:::\n\n:::unknown-directive{x=\"1\" y=\"2\"}\nAnother unknown.\n:::\n\n:::illustration{title=\"Calvin on Faith\"}\nFaith alone justifies.\n:::";
        let parsed = parse_directives(input);
        assert_eq!(parsed.len(), 5);
        assert!(parsed[0].known); // movement
        assert!(!parsed[1].known); // custom-block
        assert!(parsed[2].known); // application
        assert!(!parsed[3].known); // unknown-directive
        assert!(parsed[4].known); // illustration
        assert_eq!(parsed[1].attr("foo"), Some("bar"));
        assert_eq!(parsed[3].attr("x"), Some("1"));
        assert_eq!(parsed[3].attr("y"), Some("2"));
        let serialized = serialize_directives(&parsed);
        assert!(serialized.contains(parsed[1].raw_source.as_str()));
        assert!(serialized.contains(parsed[3].raw_source.as_str()));
        // Ordering preserved.
        let pos = |needle: &str| serialized.find(needle).unwrap();
        assert!(pos(":::movement") < pos(":::custom-block"));
        assert!(pos(":::custom-block") < pos(":::application"));
        assert!(pos(":::application") < pos(":::unknown-directive"));
        assert!(pos(":::unknown-directive") < pos(":::illustration"));
    }

    #[test]
    fn directives_embedded_in_markdown_are_found() {
        let input = "# Sermon Title\n\nSome introductory prose before the first directive.\n\n:::movement{title=\"The Word became flesh\"}\nThe incarnation is the hinge of redemptive history.\n:::\n\nMore prose between directives.\n\n- A bullet point\n- Another bullet point\n\n:::illustration{title=\"Bread of Life\"}\nJesus said: I am the bread of life.\n:::\n\nConcluding prose after the last directive.";
        let parsed = parse_directives(input);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "movement");
        assert_eq!(parsed[1].name, "illustration");
    }

    #[test]
    fn empty_and_directive_free_input_parse_cleanly() {
        assert!(parse_directives("").is_empty());
        let no_dirs = parse_directives("# Just a heading\n\nSome prose without any directives.");
        assert!(no_dirs.is_empty());
    }

    #[test]
    fn bare_unknown_round_trips_byte_for_byte() {
        let input = ":::bare-unknown\nBody content here.\n:::";
        let d = &parse_directives(input)[0];
        assert!(!d.known);
        assert_eq!(d.name, "bare-unknown");
        assert_eq!(serialize_directives(&[d.clone()]), input);
    }

    #[test]
    fn multiple_unknown_directives_all_survive_byte_exact() {
        let input = ":::alpha{a=\"1\"}\nAlpha body.\n:::\n\n:::beta{b=\"2\"}\nBeta body.\n:::";
        let parsed = parse_directives(input);
        assert_eq!(parsed.len(), 2);
        let serialized = serialize_directives(&parsed);
        assert!(serialized.contains(parsed[0].raw_source.as_str()));
        assert!(serialized.contains(parsed[1].raw_source.as_str()));
    }

    #[test]
    fn known_directives_reconstruct_header() {
        let input = ":::movement{title=\"M1\"}\nBody.\n:::\n\n:::application{point=\"2\"}\nApp.\n:::";
        let parsed = parse_directives(input);
        let serialized = serialize_directives(&parsed);
        assert!(serialized.contains(":::movement{title=\"M1\"}"));
        assert!(serialized.contains(":::application{point=\"2\"}"));
    }

    #[test]
    fn invented_directives_are_unknown_not_reintroduced() {
        // big-idea / note / scripture / warrant as *directives* must stay
        // unknown; the canonical model keeps Big Idea as sermon metadata.
        let input = ":::big-idea\nGod is faithful.\n:::\n\n:::note\nScratch.\n:::\n\n:::scripture\nJohn 3:16\n:::";
        let parsed = parse_directives(input);
        assert_eq!(parsed.len(), 3);
        for d in &parsed {
            assert!(!d.known, "{} must be unknown", d.name);
        }
        let serialized = serialize_directives(&parsed);
        assert_eq!(serialized, input);
    }

    #[test]
    fn unterminated_fence_is_ordinary_markdown() {
        let input = ":::movement{title=\"orphan\"}\nNever closed.\n\nStill prose.";
        assert!(parse_directives(input).is_empty());
    }

    #[test]
    fn attribute_order_and_duplicates_preserved() {
        let input = ":::movement{b=\"2\" a=\"1\" b=\"3\"}\nBody.\n:::";
        let d = &parse_directives(input)[0];
        let keys: Vec<&str> = d.attributes.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(keys, vec!["b", "a", "b"]);
        assert_eq!(d.attributes[2].value, "3");
    }

    #[test]
    fn crlf_source_stays_byte_exact_for_unknown() {
        let input = ":::custom-block{foo=\"bar\"}\r\nBody line.\r\n:::";
        let d = &parse_directives(input)[0];
        assert_eq!(d.raw_source, input);
        assert_eq!(d.to_markdown(), input);
        assert_eq!(d.body, "Body line.");
    }

    #[test]
    fn bare_keys_reemit_bare() {
        let input = ":::custom{flag}\nBody.\n:::";
        let d = &parse_directives(input)[0];
        assert_eq!(d.attr("flag"), Some(""));
        assert!(d.attributes[0].bare);
        assert_eq!(d.raw_source, input);
        assert_eq!(serialize_directives(&[d.clone()]), input);
    }

    #[test]
    fn unquoted_attribute_values_are_canonical() {
        // Canonical directive syntax allows `order=1` without quotes.
        let input = ":::movement{order=1 title=\"Remain connected to Christ\" warrant=\"John 15:1-4\"}\nBody.\n:::";
        let d = &parse_directives(input)[0];
        assert!(d.known);
        assert_eq!(d.attr("order"), Some("1"));
        assert_eq!(d.attr("title"), Some("Remain connected to Christ"));
        assert_eq!(d.attr("warrant"), Some("John 15:1-4"));
        // Semantic reconstruction quotes values; second pass is stable.
        let once = d.to_markdown();
        let reparsed = parse_directives(&once);
        let d2 = &reparsed[0];
        assert_eq!(d2.to_markdown(), once);
    }
}
