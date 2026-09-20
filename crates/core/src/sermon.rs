//! Sermon document model: YAML frontmatter + CommonMark body.
//!
//! On-disk format (the canonical source of truth):
//!
//! ```markdown
//! ---
//! id: rom8-28-30-groaning-glory
//! title: "Groaning for Glory"
//! date_preached: 2024-07-14
//! series: "Romans: The Gospel of God"
//! liturgical_season: Ordinary
//! primary_passage: "Rom.8.28-Rom.8.30"
//! exegetical_proposition: "Paul assures believers that God's purpose..."
//! big_idea: "God works all things for the good of those He calls."
//! structure_type: verse_by_verse
//! illustrations:
//!   - "The farmer and the seed"
//! ---
//!
//! # Groaning for Glory
//! ...body...
//! ```

use crate::directive::{self, Directive, DirectiveAttribute, SourceSpan};
use crate::error::Result;
use crate::reference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Frontmatter fields recognized by the indexer. Unknown fields are preserved
/// on disk but ignored by the index.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub date_preached: Option<String>,
    #[serde(default)]
    pub series: Option<String>,
    #[serde(default)]
    pub liturgical_season: Option<String>,
    #[serde(default)]
    pub primary_passage: Option<String>,
    #[serde(default)]
    pub exegetical_proposition: Option<String>,
    #[serde(default)]
    pub big_idea: Option<String>,
    #[serde(default)]
    pub structure_type: Option<String>,
    #[serde(default)]
    pub illustrations: Vec<String>,
}

/// A fully parsed sermon document.
#[derive(Debug, Clone)]
pub struct SermonDoc {
    pub frontmatter: Frontmatter,
    pub body: String,
    pub raw: String,
    pub file_hash: String,
}

impl SermonDoc {
    /// Parse a raw `.md` file into frontmatter + body.
    pub fn parse(raw: &str) -> Result<Self> {
        let file_hash = sha256_hex(raw.as_bytes());
        let (fm_str, body) = split_frontmatter(raw)?;
        let frontmatter: Frontmatter = if fm_str.trim().is_empty() {
            Frontmatter::default()
        } else {
            serde_yaml::from_str(fm_str)?
        };
        Ok(SermonDoc {
            frontmatter,
            body: body.to_string(),
            raw: raw.to_string(),
            file_hash,
        })
    }

    /// The id used as the primary key. Falls back to a slug of the title, then
    /// to the file hash prefix.
    pub fn resolved_id(&self) -> String {
        if let Some(id) = &self.frontmatter.id {
            if !id.trim().is_empty() {
                return id.trim().to_string();
            }
        }
        if let Some(title) = &self.frontmatter.title {
            let slug = slugify(title);
            if !slug.is_empty() {
                return slug;
            }
        }
        format!("sermon-{}", &self.file_hash[..12])
    }

    pub fn title(&self) -> String {
        self.frontmatter
            .title
            .clone()
            .unwrap_or_else(|| "(untitled)".to_string())
    }

    /// Normalized primary passage. Required for indexing; falls back to the
    /// first scripture reference found in the body.
    pub fn primary_passage(&self) -> String {
        if let Some(p) = &self.frontmatter.primary_passage {
            if !p.trim().is_empty() {
                return reference::normalize(p);
            }
        }
        // Fallback: first reference-looking token in the body.
        extract_references(&self.body)
            .into_iter()
            .next()
            .map(|r| r.canonical())
            .unwrap_or_default()
    }

    pub fn big_idea(&self) -> String {
        self.frontmatter
            .big_idea
            .clone()
            .unwrap_or_else(|| self.title())
    }

    pub fn structure_type(&self) -> String {
        self.frontmatter
            .structure_type
            .clone()
            .unwrap_or_else(|| "verse_by_verse".to_string())
    }

    /// All scripture references mentioned anywhere in the document
    /// (frontmatter primary passage + body).
    pub fn all_references(&self) -> Vec<reference::PassageRef> {
        let mut refs = Vec::new();
        if let Some(p) = &self.frontmatter.primary_passage {
            if let Ok(pr) = reference::parse_passage(p) {
                refs.push(pr);
            }
        }
        refs.extend(extract_references(&self.body));
        refs
    }
}

/// Split a document into (frontmatter, body). Frontmatter must be delimited by
/// a leading `---` line and a closing `---` line.
fn split_frontmatter(raw: &str) -> Result<(&str, &str)> {
    let trimmed = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    if !trimmed.starts_with("---") {
        return Ok(("", trimmed));
    }
    // Find the end of the first line.
    let after_first = match trimmed.find('\n') {
        Some(i) => &trimmed[i + 1..],
        None => return Ok(("", "")),
    };
    // Find the closing delimiter line.
    let mut offset = 0usize;
    for line in after_first.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if content.trim() == "---" {
            let fm = &after_first[..offset];
            let body = &after_first[offset + line.len()..];
            return Ok((fm, body));
        }
        offset += line.len();
    }
    // No closing delimiter: treat whole thing as body.
    Ok(("", trimmed))
}

/// Extract scripture references from free text. Matches patterns like
/// "Romans 8:28", "Rom. 8:28-30", "1 Cor 13:4-7", "Psalm 23".
pub fn extract_references(text: &str) -> Vec<reference::PassageRef> {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // Book name (optionally numbered) + chapter[:verse[-verse]].
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?x)
            \b
            (?:[1-3]\s*)?
            (?:Gen|Genesis|Exod|Exodus|Lev|Leviticus|Num|Numbers|Deut|Deuteronomy|
               Josh|Joshua|Judg|Judges|Ruth|1\s*Sam|2\s*Sam|1\s*Samuel|2\s*Samuel|
               1\s*Kgs|2\s*Kgs|1\s*Kings|2\s*Kings|1\s*Chr|2\s*Chr|1\s*Chronicles|2\s*Chronicles|
               Ezra|Neh|Nehemiah|Esth|Esther|Job|Ps|Psalm|Psalms|Prov|Proverbs|
               Eccl|Ecclesiastes|Song|Song\s*of\s*Solomon|Isa|Isaiah|Jer|Jeremiah|
               Lam|Lamentations|Ezek|Ezekiel|Dan|Daniel|Hos|Hosea|Joel|Amos|Obad|Obadiah|
               Jonah|Mic|Micah|Nah|Nahum|Hab|Habakkuk|Zeph|Zephaniah|Hag|Haggai|
               Zech|Zechariah|Mal|Malachi|Matt|Matthew|Mark|Luke|John|Acts|Rom|Romans|
               1\s*Cor|2\s*Cor|1\s*Corinthians|2\s*Corinthians|Gal|Galatians|Eph|Ephesians|
               Phil|Philippians|Col|Colossians|1\s*Thess|2\s*Thess|1\s*Thessalonians|2\s*Thessalonians|
               1\s*Tim|2\s*Tim|1\s*Timothy|2\s*Timothy|Titus|Phlm|Philemon|Heb|Hebrews|
               Jas|James|1\s*Pet|2\s*Pet|1\s*Peter|2\s*Peter|1\s*John|2\s*John|3\s*John|
               Jude|Rev|Revelation)
            \.?\s*
            \d{1,3}
            (?::\d{1,3}(?:\s*[-–]\s*(?:\d{1,3}:)?\d{1,3})?)?
            ",
        )
        .unwrap()
    });

    let mut out = Vec::new();
    for m in RE.find_iter(text) {
        if let Ok(p) = reference::parse_passage(m.as_str()) {
            out.push(p);
        }
    }
    out
}

/// Slugify a title into a filesystem/id-safe token.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Render a new sermon template with frontmatter, for the "new sermon" action.
pub fn new_sermon_template(title: &str, passage: &str) -> String {
    let id = slugify(title);
    let passage_norm = reference::normalize(passage);
    format!(
        "---\nid: {id}\ntitle: \"{title}\"\ndate_preached: \nseries: \nliturgical_season: \nprimary_passage: \"{passage_norm}\"\nexegetical_proposition: \nbig_idea: \nstructure_type: verse_by_verse\nillustrations: []\n---\n\n# {title}\n\n## Text\n\n> {passage}\n\n## Exegetical Proposition\n\n\n## Big Idea\n\n\n## Outline\n\n1. \n\n## Body\n\n\n## Application\n\n\n## Illustration\n\n\n## Prayer\n\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nid: test-1\ntitle: \"Test Sermon\"\ndate_preached: 2024-01-01\nprimary_passage: \"Rom.8.28-Rom.8.30\"\nbig_idea: \"God is faithful.\"\nstructure_type: verse_by_verse\nillustrations:\n  - \"The farmer\"\n---\n\n# Test\n\nSee also John 3:16 and Psalm 23.\n";

    #[test]
    fn parses_frontmatter_and_body() {
        let doc = SermonDoc::parse(SAMPLE).unwrap();
        assert_eq!(doc.resolved_id(), "test-1");
        assert_eq!(doc.title(), "Test Sermon");
        assert_eq!(doc.primary_passage(), "Rom.8.28-Rom.8.30");
        assert!(doc.body.contains("John 3:16"));
    }

    #[test]
    fn extracts_refs_from_body() {
        let refs = extract_references("See John 3:16 and Psalm 23 and 1 Cor 13:4-7.");
        let canon: Vec<String> = refs.iter().map(|r| r.canonical()).collect();
        assert!(canon.contains(&"John.3.16".to_string()));
        assert!(canon.contains(&"Ps.23.1".to_string()));
        assert!(canon.contains(&"1Cor.13.4-1Cor.13.7".to_string()));
    }

    #[test]
    fn hash_is_stable() {
        let a = sha256_hex(b"hello");
        let b = sha256_hex(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }
}

// ======================================================================
// Canonical sermon AST (Track B)
// ======================================================================
//
// Markdown remains the authoritative on-disk format. [`Sermon`] is the
// canonical in-memory/compiler representation of a Markdown sermon source:
//
// ```text
// Markdown source → Sermon (AST) → Markdown serialization
// ```
//
// Archival guarantees:
// - Unknown directives are preserved byte-exactly via their raw source.
// - Unknown attributes on known directives survive round-trip in order.
// - Ordinary Markdown between directives is kept as verbatim source.
// - Known directives are semantically normalized only as documented on each
//   type.
//
// Big Idea is sermon *metadata* (frontmatter `big_idea`), never a fenced
// directive; directive-named `big-idea` blocks stay unknown/archival.

/// Body of a [`Movement`], supporting all three preaching workflows without
/// information loss:
///
/// - manuscript style: `manuscript` set, `bullets` empty
/// - outline style: `bullets` non-empty, `manuscript` unset
/// - combined style: both set (manuscript prose plus outline bullets)
///
/// Normalization note: within one movement, prose paragraphs are collected
/// in order and list items in order; the prose/bullet interleaving of the
/// original source is not preserved (manuscript is emitted before bullets).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovementBody {
    /// Prose manuscript paragraphs, joined with blank lines. `None` when the
    /// movement is pure outline style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manuscript: Option<String>,
    /// Outline bullets, in source order. Markers (`-`, `*`, `+`, or
    /// `1.`/`1)`) are stripped; item text is trimmed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bullets: Vec<String>,
}

impl MovementBody {
    /// Manuscript style: prose only.
    pub fn is_manuscript_style(&self) -> bool {
        self.manuscript.is_some() && self.bullets.is_empty()
    }

    /// Outline style: bullets only.
    pub fn is_outline_style(&self) -> bool {
        self.manuscript.is_none() && !self.bullets.is_empty()
    }

    /// Combined style: prose and bullets.
    pub fn is_combined_style(&self) -> bool {
        self.manuscript.is_some() && !self.bullets.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.manuscript.is_none() && self.bullets.is_empty()
    }
}

/// A structured `movement` directive: one major movement of the sermon.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Movement {
    /// Movement order. Consumed from the `order` attribute, with `index`
    /// accepted as a legacy fallback (re-emitted canonically as `order`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Raw warrant string (e.g. `"John 15:1-4"`). Scripture resolution is
    /// Track A's job; this field deliberately carries the unresolved string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warrant: Option<String>,
    pub body: MovementBody,
    /// Attributes the canonical model does not consume; preserved verbatim
    /// (order and duplicates intact) so they survive round-trip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<DirectiveAttribute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

/// An `illustration` directive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Illustration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Body text (trimmed semantic view; the raw block is in `raw_source`).
    pub body: String,
    /// Unknown attributes, preserved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<DirectiveAttribute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
    /// Original raw source of the whole directive block.
    pub raw_source: String,
}

/// An `application` directive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Application {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    pub body: String,
    /// Unknown attributes, preserved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<DirectiveAttribute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
    pub raw_source: String,
}

/// An `exegetical-notes` directive: private study material.
///
/// The dedicated variant (and [`Block::is_private_notes`]) gives later
/// export code a reliable semantic way to exclude these notes by default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExegeticalNotes {
    pub body: String,
    /// Unknown attributes, preserved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<DirectiveAttribute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
    pub raw_source: String,
}

/// Ordinary Markdown between directives, kept verbatim for archival safety.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownBlock {
    /// Raw Markdown source of this segment, exactly as it appeared
    /// (paragraphs, headings, lists, blockquotes, emphasis, code, and blank
    /// lines included).
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

/// One top-level block of a canonical sermon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// Ordinary Markdown (always verbatim source).
    Markdown(MarkdownBlock),
    Movement(Movement),
    Illustration(Illustration),
    Application(Application),
    /// Private study notes; excluded from export by default downstream.
    ExegeticalNotes(ExegeticalNotes),
    /// Unknown directive: archival, byte-exact raw source.
    Unknown(Directive),
}

impl Block {
    /// Source location, where practical.
    pub fn span(&self) -> Option<SourceSpan> {
        match self {
            Block::Markdown(b) => b.span,
            Block::Movement(m) => m.span,
            Block::Illustration(i) => i.span,
            Block::Application(a) => a.span,
            Block::ExegeticalNotes(n) => n.span,
            Block::Unknown(d) => Some(d.span),
        }
    }

    /// True for `exegetical-notes`: private material that export code must
    /// exclude by default.
    pub fn is_private_notes(&self) -> bool {
        matches!(self, Block::ExegeticalNotes(_))
    }

    /// Directive name when this block is an unknown directive.
    pub fn unknown_directive_name(&self) -> Option<&str> {
        match self {
            Block::Unknown(d) => Some(d.name.as_str()),
            _ => None,
        }
    }

    /// Serialize this block back to Markdown. Unknown directives emit their
    /// raw source verbatim; Markdown segments emit their verbatim source;
    /// known directives are reconstructed from typed parts plus preserved
    /// unknown attributes.
    pub fn to_markdown(&self) -> String {
        match self {
            Block::Markdown(b) => b.source.clone(),
            Block::Unknown(d) => d.raw_source.clone(),
            Block::Movement(m) => movement_to_markdown(m),
            Block::Illustration(i) => {
                known_to_markdown("illustration", &illustration_attrs(i), &i.body)
            }
            Block::Application(a) => {
                known_to_markdown("application", &application_attrs(a), &a.body)
            }
            Block::ExegeticalNotes(n) => {
                known_to_markdown("exegetical-notes", &notes_attrs(n), &n.body)
            }
        }
    }
}

/// The canonical sermon: metadata plus an ordered block list.
///
/// Markdown is the source of truth; `Sermon` is its durable in-memory
/// representation. Existing consumers of [`SermonDoc`] are unaffected — this
/// is purely additive.
#[derive(Debug, Clone, Serialize)]
pub struct Sermon {
    /// Sermon metadata (frontmatter): title, date, series, primary passage,
    /// Big Idea, etc.
    pub meta: Frontmatter,
    /// Ordered body blocks: Markdown, known directives, unknown directives.
    pub blocks: Vec<Block>,
    /// Verbatim YAML frontmatter source, kept for archival round-trip.
    frontmatter_raw: String,
    has_frontmatter: bool,
}

impl Sermon {
    /// Parse a full Markdown sermon document into the canonical AST.
    pub fn parse(raw: &str) -> Result<Sermon> {
        let (fm_str, body) = split_frontmatter(raw)?;
        let has_frontmatter = !fm_str.trim().is_empty();
        let meta: Frontmatter = if has_frontmatter {
            serde_yaml::from_str(fm_str)?
        } else {
            Frontmatter::default()
        };
        Ok(Sermon {
            meta,
            blocks: parse_blocks(body),
            frontmatter_raw: fm_str.to_string(),
            has_frontmatter,
        })
    }

    /// Serialize back to canonical Markdown. Documents containing no known
    /// directives round-trip byte-exactly; documents with known directives
    /// receive the documented semantic normalization and are stable under a
    /// second pass.
    pub fn to_markdown(&self) -> String {
        let mut body = String::new();
        for block in &self.blocks {
            body.push_str(&block.to_markdown());
        }
        if self.has_frontmatter {
            format!("---\n{}---\n{}", self.frontmatter_raw, body)
        } else {
            body
        }
    }

    /// Big Idea — sermon metadata, not a directive.
    pub fn big_idea(&self) -> Option<&str> {
        self.meta.big_idea.as_deref()
    }

    /// All structured movements, in document order.
    pub fn movements(&self) -> impl Iterator<Item = &Movement> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Movement(m) => Some(m),
            _ => None,
        })
    }

    pub fn illustrations(&self) -> impl Iterator<Item = &Illustration> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Illustration(i) => Some(i),
            _ => None,
        })
    }

    pub fn applications(&self) -> impl Iterator<Item = &Application> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Application(a) => Some(a),
            _ => None,
        })
    }

    /// Exegetical notes blocks (private study material).
    pub fn exegetical_notes(&self) -> impl Iterator<Item = &ExegeticalNotes> {
        self.blocks.iter().filter_map(|b| match b {
            Block::ExegeticalNotes(n) => Some(n),
            _ => None,
        })
    }

    /// Private study material only. This is the semantic hook for later
    /// export code to exclude private notes by default.
    pub fn private_notes(&self) -> impl Iterator<Item = &ExegeticalNotes> {
        self.exegetical_notes()
    }

    /// Unknown directives (archival blocks), in document order.
    pub fn unknown_directives(&self) -> impl Iterator<Item = &Directive> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Unknown(d) => Some(d),
            _ => None,
        })
    }
}

/// Split a body into ordered blocks: verbatim Markdown gaps plus typed
/// directives.
pub fn parse_blocks(body: &str) -> Vec<Block> {
    let directives = directive::parse_directives(body);
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    for d in directives {
        if d.span.start > cursor {
            blocks.push(Block::Markdown(MarkdownBlock {
                source: body[cursor..d.span.start].to_string(),
                span: Some(SourceSpan::new(cursor, d.span.start)),
            }));
        }
        let end = d.span.end;
        blocks.push(typed_block(d));
        cursor = end;
    }
    if cursor < body.len() {
        blocks.push(Block::Markdown(MarkdownBlock {
            source: body[cursor..].to_string(),
            span: Some(SourceSpan::new(cursor, body.len())),
        }));
    }
    blocks
}

/// Convert a parsed directive into its typed block. Unknown directives pass
/// through untouched (archival).
fn typed_block(d: Directive) -> Block {
    if !d.known {
        return Block::Unknown(d);
    }
    match d.name.as_str() {
        "movement" => {
            let mut d = d;
            let order = d
                .take_attr(&["order"])
                .or_else(|| d.take_attr(&["index"]))
                .and_then(|v| v.trim().parse::<u32>().ok());
            let title = d.take_attr(&["title"]);
            let warrant = d.take_attr(&["warrant"]);
            let body = split_movement_body(&d.body);
            let span = Some(d.span);
            Block::Movement(Movement {
                order,
                title,
                warrant,
                body,
                attributes: d.attributes,
                span,
            })
        }
        "illustration" => {
            let mut d = d;
            let id = d.take_attr(&["id"]);
            let title = d.take_attr(&["title"]);
            Block::Illustration(Illustration {
                id,
                title,
                body: d.body.trim().to_string(),
                attributes: d.attributes,
                span: Some(d.span),
                raw_source: d.raw_source,
            })
        }
        "application" => {
            let mut d = d;
            let audience = d.take_attr(&["audience"]);
            Block::Application(Application {
                audience,
                body: d.body.trim().to_string(),
                attributes: d.attributes,
                span: Some(d.span),
                raw_source: d.raw_source,
            })
        }
        _ => {
            // "exegetical-notes"
            let d = d;
            Block::ExegeticalNotes(ExegeticalNotes {
                body: d.body.trim().to_string(),
                attributes: d.attributes,
                span: Some(d.span),
                raw_source: d.raw_source,
            })
        }
    }
}

/// Split movement body text into manuscript paragraphs and outline bullets.
fn split_movement_body(body: &str) -> MovementBody {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut bullets: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();

    fn flush(current: &mut Vec<String>, paragraphs: &mut Vec<String>) {
        if !current.is_empty() {
            paragraphs.push(current.join("\n"));
            current.clear();
        }
    }

    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            flush(&mut current, &mut paragraphs);
        } else if let Some(item) = strip_bullet_marker(t) {
            flush(&mut current, &mut paragraphs);
            bullets.push(item);
        } else {
            current.push(t.to_string());
        }
    }
    flush(&mut current, &mut paragraphs);

    MovementBody {
        manuscript: if paragraphs.is_empty() {
            None
        } else {
            Some(paragraphs.join("\n\n"))
        },
        bullets,
    }
}

/// Strip a list marker (`-`, `*`, `+`, `1.`, `1)`) from a trimmed line.
fn strip_bullet_marker(t: &str) -> Option<String> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(marker) {
            return Some(rest.trim().to_string());
        }
    }
    let (first, rest) = t.split_once(char::is_whitespace)?;
    if !first.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !(first.ends_with('.') || first.ends_with(')')) {
        return None;
    }
    if rest.trim().is_empty() {
        return None;
    }
    Some(rest.trim().to_string())
}

fn movement_to_markdown(m: &Movement) -> String {
    let mut attrs: Vec<String> = Vec::new();
    if let Some(order) = m.order {
        attrs.push(format!("order=\"{order}\""));
    }
    if let Some(title) = &m.title {
        attrs.push(format!("title=\"{title}\""));
    }
    if let Some(warrant) = &m.warrant {
        attrs.push(format!("warrant=\"{warrant}\""));
    }
    attrs.extend(m.attributes.iter().map(|a| a.to_source()));

    let mut body = String::new();
    if let Some(manuscript) = &m.body.manuscript {
        body.push_str(manuscript.trim());
    }
    for bullet in &m.body.bullets {
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str("- ");
        body.push_str(bullet);
    }
    known_to_markdown("movement", &attrs, &body)
}

fn illustration_attrs(i: &Illustration) -> Vec<String> {
    let mut attrs = Vec::new();
    if let Some(id) = &i.id {
        attrs.push(format!("id=\"{id}\""));
    }
    if let Some(title) = &i.title {
        attrs.push(format!("title=\"{title}\""));
    }
    attrs.extend(i.attributes.iter().map(|a| a.to_source()));
    attrs
}

fn application_attrs(a: &Application) -> Vec<String> {
    let mut attrs = Vec::new();
    if let Some(audience) = &a.audience {
        attrs.push(format!("audience=\"{audience}\""));
    }
    attrs.extend(a.attributes.iter().map(|x| x.to_source()));
    attrs
}

fn notes_attrs(n: &ExegeticalNotes) -> Vec<String> {
    n.attributes.iter().map(|a| a.to_source()).collect()
}

/// Reconstruct a known directive block (no trailing newline; newlines
/// following the block are part of the surrounding verbatim Markdown gaps).
fn known_to_markdown(name: &str, attrs: &[String], body: &str) -> String {
    let mut out = String::new();
    out.push_str(":::");
    out.push_str(name);
    if !attrs.is_empty() {
        out.push('{');
        out.push_str(&attrs.join(" "));
        out.push('}');
    }
    out.push('\n');
    out.push_str(body.trim());
    out.push_str("\n:::");
    out
}

#[cfg(test)]
mod ast_tests {
    use super::*;

    const FULL_DOC: &str = "---\nid: vine-sermon\ntitle: \"Remain in Christ\"\ndate_preached: 2024-07-14\nseries: \"Abide\"\nprimary_passage: \"John.15.1-4\"\nbig_idea: \"Remaining in Christ is the source of fruitfulness.\"\nstructure_type: verse_by_verse\nillustrations:\n  - \"The pruned vine\"\n---\n\n# Remain in Christ\n\nIntro prose with *emphasis* and **strong** and `inline code`.\n\n> A blockquote stays untouched.\n\n:::movement{order=1 title=\"Remain connected to Christ\" warrant=\"John 15:1-4\"}\nManuscript paragraph one.\nManuscript paragraph one continues.\n\nManuscript paragraph two.\n:::\n\nMiddle prose.\n\n:::movement{order=2 title=\"Pruned for fruit\"}\n- First bullet\n- Second bullet\n:::\n\n:::illustration{id=\"vineyard-pruning\" title=\"Pruning a fruit tree\"}\nIllustration body.\n:::\n\n:::application{audience=\"congregation\"}\nApplication body.\n:::\n\n:::exegetical-notes\nPrivate study notes.\n:::\n\n:::custom-block{foo=\"bar\"}\nOriginal body.\n:::\n\nClosing prose.\n";

    #[test]
    fn parses_metadata_and_big_idea_as_metadata() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        assert_eq!(s.meta.id.as_deref(), Some("vine-sermon"));
        assert_eq!(s.meta.title.as_deref(), Some("Remain in Christ"));
        assert_eq!(s.meta.series.as_deref(), Some("Abide"));
        assert_eq!(s.meta.primary_passage.as_deref(), Some("John.15.1-4"));
        assert_eq!(
            s.big_idea(),
            Some("Remaining in Christ is the source of fruitfulness.")
        );
    }

    #[test]
    fn movement_typed_with_order_title_warrant_and_manuscript() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        let movements: Vec<&Movement> = s.movements().collect();
        assert_eq!(movements.len(), 2);
        let m = movements[0];
        assert_eq!(m.order, Some(1));
        assert_eq!(m.title.as_deref(), Some("Remain connected to Christ"));
        assert_eq!(m.warrant.as_deref(), Some("John 15:1-4"));
        assert!(m.body.is_manuscript_style());
        assert!(m
            .body
            .manuscript
            .as_ref()
            .unwrap()
            .contains("Manuscript paragraph one."));
        // Warrant carried raw; no Scripture parsing here (Track A).
    }

    #[test]
    fn movement_outline_style_collects_bullets() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        let m = s.movements().nth(1).unwrap();
        assert!(m.body.is_outline_style());
        assert_eq!(m.body.bullets, vec!["First bullet", "Second bullet"]);
    }

    #[test]
    fn combined_style_supported_without_information_loss() {
        let doc = ":::movement{order=1 title=\"Combined\"}\nProse here.\n\n- Bullet A\n- Bullet B\n:::";
        let s = Sermon::parse(doc).unwrap();
        let m = s.movements().next().unwrap();
        assert!(m.body.is_combined_style());
        assert_eq!(m.body.manuscript.as_deref(), Some("Prose here."));
        assert_eq!(m.body.bullets, vec!["Bullet A", "Bullet B"]);
    }

    #[test]
    fn index_attribute_accepted_as_legacy_order() {
        let doc = ":::movement{index=\"3\" title=\"Legacy\"}\nBody.\n:::";
        let s = Sermon::parse(doc).unwrap();
        let m = s.movements().next().unwrap();
        assert_eq!(m.order, Some(3));
        // Re-emitted canonically as order.
        assert!(s.to_markdown().contains("order=\"3\""));
    }

    #[test]
    fn illustration_application_notes_typed() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        let ill = s.illustrations().next().unwrap();
        assert_eq!(ill.id.as_deref(), Some("vineyard-pruning"));
        assert_eq!(ill.title.as_deref(), Some("Pruning a fruit tree"));
        assert_eq!(ill.body, "Illustration body.");

        let app = s.applications().next().unwrap();
        assert_eq!(app.audience.as_deref(), Some("congregation"));
        assert_eq!(app.body, "Application body.");

        let notes = s.exegetical_notes().next().unwrap();
        assert_eq!(notes.body, "Private study notes.");
    }

    #[test]
    fn private_notes_semantically_distinguishable_for_export() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        let privates: Vec<&ExegeticalNotes> = s.private_notes().collect();
        assert_eq!(privates.len(), 1);
        assert!(privates[0].body.contains("Private study notes"));
        // Only exegetical-notes blocks are private.
        for block in &s.blocks {
            let private = block.is_private_notes();
            assert_eq!(private, matches!(block, Block::ExegeticalNotes(_)));
        }
    }

    #[test]
    fn unknown_directive_survives_full_document_round_trip() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        let unknowns: Vec<&Directive> = s.unknown_directives().collect();
        assert_eq!(unknowns.len(), 1);
        assert_eq!(unknowns[0].name, "custom-block");
        assert_eq!(unknowns[0].attr("foo"), Some("bar"));
        // Raw source preserved byte-exactly.
        assert_eq!(
            unknowns[0].raw_source,
            ":::custom-block{foo=\"bar\"}\nOriginal body.\n:::"
        );
        // ... and it appears verbatim in the serialized document.
        assert!(s.to_markdown().contains(unknowns[0].raw_source.as_str()));
    }

    #[test]
    fn unknown_only_document_round_trips_byte_exact() {
        let doc = "---\nid: archival\ntitle: \"Archival\"\n---\n\n# Heading\n\nProse with *emphasis*.\n\n:::custom-block{foo=\"bar\"}\nOriginal body.\n:::\n\n:::bare\nBare body.\n:::\n\n> A blockquote.\n\n```code\nfenced();\n```\n\nTrailing paragraph.\n";
        let s = Sermon::parse(doc).unwrap();
        assert_eq!(s.to_markdown(), doc);
    }

    #[test]
    fn no_frontmatter_document_round_trips_byte_exact() {
        let doc = "Just prose.\n\n:::unknown-thing{a=\"1\"}\nKeep me.\n:::\n";
        let s = Sermon::parse(doc).unwrap();
        assert_eq!(s.to_markdown(), doc);
    }

    #[test]
    fn unknown_attributes_on_known_directive_survive_round_trip() {
        let doc = "---\ntitle: \"T\"\n---\n\n:::movement{title=\"Christ is Lord\" version=\"2\"}\nBody.\n:::\n";
        let s = Sermon::parse(doc).unwrap();
        let m = s.movements().next().unwrap();
        assert_eq!(m.title.as_deref(), Some("Christ is Lord"));
        assert_eq!(m.attributes.len(), 1);
        assert_eq!(m.attributes[0].key, "version");
        assert_eq!(m.attributes[0].value, "2");
        assert!(s.to_markdown().contains("version=\"2\""));
    }

    #[test]
    fn ordinary_markdown_around_directives_preserved() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        let md: Vec<&MarkdownBlock> = s
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Markdown(m) => Some(m),
                _ => None,
            })
            .collect();
        // Intro, middle, closing markdown segments.
        assert!(md.len() >= 3);
        let intro = &md[0].source;
        assert!(intro.contains("# Remain in Christ"));
        assert!(intro.contains("*emphasis*"));
        assert!(intro.contains("**strong**"));
        assert!(intro.contains("`inline code`"));
        assert!(intro.contains("> A blockquote stays untouched."));
    }

    #[test]
    fn block_ordering_preserved() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        let kinds: Vec<&str> = s
            .blocks
            .iter()
            .filter(|b| !matches!(b, Block::Markdown(_)))
            .map(|b| match b {
                Block::Movement(_) => "movement",
                Block::Illustration(_) => "illustration",
                Block::Application(_) => "application",
                Block::ExegeticalNotes(_) => "exegetical-notes",
                Block::Unknown(_) => "unknown",
                Block::Markdown(_) => unreachable!(),
            })
            .collect();
        assert_eq!(
            kinds,
            vec!["movement", "movement", "illustration", "application", "exegetical-notes", "unknown"]
        );
    }

    #[test]
    fn serialization_is_idempotent() {
        let s1 = Sermon::parse(FULL_DOC).unwrap();
        let pass1 = s1.to_markdown();
        let s2 = Sermon::parse(&pass1).unwrap();
        let pass2 = s2.to_markdown();
        assert_eq!(pass1, pass2, "round-trip must stabilize after one pass");
    }

    #[test]
    fn known_directive_semantics_survive_round_trip() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        let out = s.to_markdown();
        let s2 = Sermon::parse(&out).unwrap();
        let m = s2.movements().next().unwrap();
        assert_eq!(m.order, Some(1));
        assert_eq!(m.title.as_deref(), Some("Remain connected to Christ"));
        assert_eq!(m.warrant.as_deref(), Some("John 15:1-4"));
        assert!(m.body.manuscript.as_ref().unwrap().contains("paragraph two"));
    }

    #[test]
    fn invented_directives_stay_unknown_archival_blocks() {
        let doc = ":::big-idea\nGod is faithful.\n:::\n\n:::warrant\nJohn 3:16\n:::";
        let s = Sermon::parse(doc).unwrap();
        let names: Vec<&str> = s
            .unknown_directives()
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(names, vec!["big-idea", "warrant"]);
        assert!(s.movements().next().is_none());
        assert_eq!(s.to_markdown(), doc, "unknown-only doc must be byte-exact");
    }

    #[test]
    fn spans_tracked_for_blocks() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        for block in &s.blocks {
            assert!(block.span().is_some());
        }
        // Spans are monotonically ordered and within the body.
        let spans: Vec<SourceSpan> = s.blocks.iter().filter_map(|b| b.span()).collect();
        for w in spans.windows(2) {
            assert!(w[0].end <= w[1].start);
        }
    }

    #[test]
    fn empty_and_plain_documents_parse() {
        let s = Sermon::parse("").unwrap();
        assert!(s.blocks.is_empty());
        assert_eq!(s.to_markdown(), "");

        let s = Sermon::parse("# Only a heading\n\nNo directives at all.\n").unwrap();
        assert_eq!(s.blocks.len(), 1);
        assert!(matches!(&s.blocks[0], Block::Markdown(_)));
        assert_eq!(s.to_markdown(), "# Only a heading\n\nNo directives at all.\n");
    }

    #[test]
    fn ast_serializes_for_later_consumers() {
        let s = Sermon::parse(FULL_DOC).unwrap();
        let json = serde_json::to_value(&s).unwrap();
        assert!(json.get("meta").is_some());
        assert!(json.get("blocks").is_some());
        let first_dir = json["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["type"] == "movement")
            .unwrap();
        assert_eq!(first_dir["order"], 1);
    }
}
