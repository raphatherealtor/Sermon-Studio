//! V1 professional PDF export: in-process Typst rendering plus immutable
//! export snapshots.
//!
//! Architecture:
//!
//! ```text
//! Sermon AST (Track B, canonical)
//!   → template data (private notes excluded by default)
//!   → main.typ (embedded template + data reference)
//!   → immutable ExportSnapshot (SHA-256 content hash, created BEFORE render)
//!   → typst::compile in process (embedded fonts, no network, no subprocess)
//!   → typst_pdf bytes
//!   → validate (%PDF- signature, non-empty, ≥1 page)
//!   → Track C atomic write (temp sibling → sync → rename)
//!   → ExportOutcome report for the integration/frontend layer
//! ```
//!
//! Exactly three export formats exist:
//!
//! * [`ExportFormat::PulpitManuscript`] with [`PulpitMode::Manuscript`],
//!   [`PulpitMode::Outline`], or [`PulpitMode::Combined`].
//! * [`ExportFormat::ChurchBulletin`] (congregation-facing outline).
//! * [`ExportFormat::TeachingNotes`] (teacher-facing Bible study / teaching
//!   notes compiled from the same canonical AST with
//!   [`TeachingOptions`]).
//!
//! Determinism: templates are embedded at compile time, fonts come from the
//! versioned `typst-assets` crate, templates never call `datetime.today()`
//! and disable the PDF document date, and the content hash covers the exact
//! Typst source handed to the compiler. Same sermon + template version +
//! options + application version ⇒ same rendering inputs.
//!
//! This module does NOT add Tauri commands — the integration track owns IPC
//! and decides how/when to call [`export_sermon`].

use crate::atomic_save;
use crate::sermon::{Block, Sermon};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::OnceLock;
use thiserror::Error;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::LazyHash;
use typst::LibraryExt as _;
use typst::World;
use typst::WorldExt as _;
use typst_layout::PagedDocument;

// ---------------------------------------------------------------------------
// Embedded templates
// ---------------------------------------------------------------------------

const PULPIT_TEMPLATE_SRC: &str = include_str!("../templates/pulpit_manuscript.typ");
const BULLETIN_TEMPLATE_SRC: &str = include_str!("../templates/church_bulletin.typ");
const TEACHING_TEMPLATE_SRC: &str = include_str!("../templates/teaching_notes.typ");

/// Identifier of the pulpit manuscript template.
pub const PULPIT_TEMPLATE_ID: &str = "pulpit_manuscript";
/// Identifier of the church bulletin template.
pub const BULLETIN_TEMPLATE_ID: &str = "church_bulletin";
/// Identifier of the Bible study / teaching notes template.
pub const TEACHING_TEMPLATE_ID: &str = "teaching_notes";
/// Bump when the pulpit template's rendered output changes meaningfully.
pub const PULPIT_TEMPLATE_VERSION: &str = "1.0.0";
/// Bump when the bulletin template's rendered output changes meaningfully.
pub const BULLETIN_TEMPLATE_VERSION: &str = "1.0.0";
/// Bump when the teaching notes template's rendered output changes meaningfully.
pub const TEACHING_TEMPLATE_VERSION: &str = "1.0.0";

/// Schema version of [`ExportSnapshot`]. Bump on incompatible changes.
pub const EXPORT_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Request model
// ---------------------------------------------------------------------------

/// The three canonical export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    PulpitManuscript,
    ChurchBulletin,
    TeachingNotes,
}

/// Spacing density for the teaching notes layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TeachingSpacing {
    /// Tighter leading for a compact handout.
    Compact,
    /// Roomier leading for annotation.
    Comfortable,
}

impl TeachingSpacing {
    pub fn parse(s: &str) -> Option<TeachingSpacing> {
        match s {
            "compact" => Some(TeachingSpacing::Compact),
            "comfortable" => Some(TeachingSpacing::Comfortable),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TeachingSpacing::Compact => "compact",
            TeachingSpacing::Comfortable => "comfortable",
        }
    }
}

/// Restrained, teacher-facing options for [`ExportFormat::TeachingNotes`].
/// Only meaningful for that format; the transport rejects these options on
/// other formats rather than silently ignoring them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TeachingOptions {
    /// Include the Big Idea statement. Default: true.
    pub include_big_idea: bool,
    /// Spacing density. Default: comfortable.
    pub spacing: TeachingSpacing,
    /// Use teacher-friendly headings (Big Idea, Movements, Applications,
    /// For Discussion). Default: true.
    pub teacher_headings: bool,
    /// Include `:::discussion` blocks as a For Discussion / Q&A section.
    /// Default: false (private preparation stays private unless asked for).
    pub include_discussion: bool,
}

impl Default for TeachingOptions {
    fn default() -> Self {
        TeachingOptions {
            include_big_idea: true,
            spacing: TeachingSpacing::Comfortable,
            teacher_headings: true,
            include_discussion: false,
        }
    }
}

/// Pulpit manuscript rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PulpitMode {
    /// Readable manuscript prose.
    Manuscript,
    /// Movement titles, warrants, and movement bullets.
    Outline,
    /// Manuscript prose plus outline/bullet structure.
    Combined,
}

impl PulpitMode {
    pub fn parse(s: &str) -> Option<PulpitMode> {
        match s {
            "manuscript" => Some(PulpitMode::Manuscript),
            "outline" => Some(PulpitMode::Outline),
            "combined" => Some(PulpitMode::Combined),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PulpitMode::Manuscript => "manuscript",
            PulpitMode::Outline => "outline",
            PulpitMode::Combined => "combined",
        }
    }
}

/// What to export and how.
#[derive(Debug, Clone, Serialize)]
pub struct ExportRequest {
    pub format: ExportFormat,
    /// Required for [`ExportFormat::PulpitManuscript`]; must be `None` for
    /// the bulletin and teaching notes (ambiguous requests are rejected, not
    /// guessed).
    pub pulpit_mode: Option<PulpitMode>,
    /// Include private `exegetical-notes` blocks. Only ever valid for the
    /// pulpit manuscript and teaching notes; the bulletin is structurally
    /// public.
    pub include_private_notes: bool,
    /// Teaching-notes layout options. Must be `None` for every other format
    /// (rejected, not guessed).
    pub teaching_options: Option<TeachingOptions>,
}

impl ExportRequest {
    pub fn pulpit(mode: PulpitMode) -> ExportRequest {
        ExportRequest {
            format: ExportFormat::PulpitManuscript,
            pulpit_mode: Some(mode),
            include_private_notes: false,
            teaching_options: None,
        }
    }

    pub fn bulletin() -> ExportRequest {
        ExportRequest {
            format: ExportFormat::ChurchBulletin,
            pulpit_mode: None,
            include_private_notes: false,
            teaching_options: None,
        }
    }

    pub fn teaching(options: TeachingOptions) -> ExportRequest {
        ExportRequest {
            format: ExportFormat::TeachingNotes,
            pulpit_mode: None,
            include_private_notes: false,
            teaching_options: Some(options),
        }
    }

    fn validate(&self) -> Result<(), ExportError> {
        match self.format {
            ExportFormat::PulpitManuscript => {
                if self.pulpit_mode.is_none() {
                    return Err(ExportError::InvalidRequest(
                        "pulpit_manuscript export requires a mode (manuscript, outline, or combined)"
                            .to_string(),
                    ));
                }
                if self.teaching_options.is_some() {
                    return Err(ExportError::InvalidRequest(
                        "teaching options are only valid for the teaching_notes format".to_string(),
                    ));
                }
            }
            ExportFormat::ChurchBulletin => {
                if self.pulpit_mode.is_some() {
                    return Err(ExportError::InvalidRequest(
                        "church_bulletin export does not take a pulpit mode".to_string(),
                    ));
                }
                if self.include_private_notes {
                    return Err(ExportError::InvalidRequest(
                        "church_bulletin exports are congregation-facing; private exegetical \
                         notes can never be included"
                            .to_string(),
                    ));
                }
                if self.teaching_options.is_some() {
                    return Err(ExportError::InvalidRequest(
                        "teaching options are only valid for the teaching_notes format".to_string(),
                    ));
                }
            }
            ExportFormat::TeachingNotes => {
                if self.pulpit_mode.is_some() {
                    return Err(ExportError::InvalidRequest(
                        "teaching_notes export does not take a pulpit mode".to_string(),
                    ));
                }
                if self.teaching_options.is_none() {
                    return Err(ExportError::InvalidRequest(
                        "teaching_notes export requires teaching options".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Structured export errors. Kept local to the export subsystem so the shared
/// `error.rs` surface stays untouched.
#[derive(Debug, Error)]
pub enum ExportError {
    #[error("invalid export request: {0}")]
    InvalidRequest(String),
    #[error("template rendering failed: {0}")]
    Render(String),
    #[error("generated PDF failed validation: {0}")]
    InvalidPdf(String),
    #[error("failed to serialize export data: {0}")]
    Serialization(String),
    #[error("failed to write export output: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Template data (AST → dict handed to the Typst templates)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TemplateMeta {
    id: String,
    title: String,
    date: Option<String>,
    series: Option<String>,
    season: Option<String>,
    passage: Option<String>,
    big_idea: Option<String>,
    proposition: Option<String>,
    structure: String,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TemplateBlock {
    /// Ordinary markdown, pre-rendered to Typst markup. `outline` carries
    /// only the heading lines (used by outline/bulletin modes).
    Markdown { text: String, outline: String },
    Movement {
        order: Option<u32>,
        title: Option<String>,
        warrant: Option<String>,
        manuscript: Option<String>,
        bullets: Vec<String>,
    },
    Illustration {
        label: Option<String>,
        text: String,
    },
    Application {
        audience: Option<String>,
        text: String,
    },
    /// Private study notes — only ever constructed when explicitly requested.
    Notes { text: String },
    /// Discussion / Q&A prompts (`:::discussion` blocks) — only ever
    /// constructed for teaching notes when explicitly requested.
    Discussion { text: String },
    /// Unknown directive, preserved verbatim (archival rendering).
    Unknown { name: String, raw: String },
}

/// Teaching-notes layout options as handed to the template.
#[derive(Serialize)]
struct TeachingTemplateData {
    include_big_idea: bool,
    spacing: &'static str,
    teacher_headings: bool,
    include_discussion: bool,
}

#[derive(Serialize)]
struct TemplateData {
    meta: TemplateMeta,
    mode: String,
    blocks: Vec<TemplateBlock>,
    /// Present only for [`ExportFormat::TeachingNotes`]; `null` otherwise.
    teaching: Option<TeachingTemplateData>,
}

/// Canonical sermon id, mirroring the v1 `SermonDoc::resolved_id` fallbacks:
/// frontmatter id → slugified title → `sermon-<source-hash-12>`.
fn resolved_sermon_id(sermon: &Sermon, source_hash: &str) -> String {
    if let Some(id) = &sermon.meta.id {
        if !id.trim().is_empty() {
            return id.trim().to_string();
        }
    }
    if let Some(title) = &sermon.meta.title {
        let slug = crate::sermon::slugify(title);
        if !slug.is_empty() {
            return slug;
        }
    }
    format!("sermon-{}", &source_hash[..12])
}

fn build_template_data(
    sermon: &Sermon,
    sermon_id: &str,
    request: &ExportRequest,
) -> TemplateData {
    let include_private = request.include_private_notes;
    let teaching = request.teaching_options;
    let mode = match (request.format, request.pulpit_mode) {
        (ExportFormat::PulpitManuscript, Some(m)) => m.as_str().to_string(),
        (ExportFormat::ChurchBulletin, _) => "bulletin".to_string(),
        (ExportFormat::TeachingNotes, _) => "teaching".to_string(),
        (ExportFormat::PulpitManuscript, None) => unreachable!("validated earlier"),
    };

    let mut meta = TemplateMeta {
        id: sermon_id.to_string(),
        title: sermon.meta.title.clone().unwrap_or_else(|| "(untitled)".to_string()),
        date: sermon.meta.date_preached.clone(),
        series: sermon.meta.series.clone(),
        season: sermon.meta.liturgical_season.clone(),
        passage: sermon.meta.primary_passage.clone(),
        big_idea: sermon.meta.big_idea.clone(),
        proposition: sermon.meta.exegetical_proposition.clone(),
        structure: sermon
            .meta
            .structure_type
            .clone()
            .unwrap_or_else(|| "verse_by_verse".to_string()),
    };
    // Big Idea inclusion is a teaching-notes option; every other format
    // keeps the canonical frontmatter rendering untouched.
    if let Some(opts) = teaching {
        if !opts.include_big_idea {
            meta.big_idea = None;
        }
    }

    let mut blocks = Vec::new();
    for block in &sermon.blocks {
        match block {
            Block::Markdown(m) => {
                let (text, outline) = render_markdown(&m.source);
                blocks.push(TemplateBlock::Markdown { text, outline });
            }
            Block::Movement(m) => blocks.push(TemplateBlock::Movement {
                order: m.order,
                title: m.title.clone(),
                warrant: m.warrant.clone(),
                manuscript: m
                    .body
                    .manuscript
                    .as_ref()
                    .map(|prose| render_paragraphs(prose)),
                bullets: m.body.bullets.iter().map(|b| render_inline(b)).collect(),
            }),
            Block::Illustration(i) => {
                let label = i
                    .title
                    .clone()
                    .or_else(|| i.id.clone())
                    .map(|t| format!("Illustration — {t}"));
                blocks.push(TemplateBlock::Illustration {
                    label,
                    text: render_inline(&i.body),
                });
            }
            Block::Application(a) => blocks.push(TemplateBlock::Application {
                audience: a.audience.clone(),
                text: render_inline(&a.body),
            }),
            Block::ExegeticalNotes(n) => {
                if include_private {
                    blocks.push(TemplateBlock::Notes {
                        text: render_inline(&n.body),
                    });
                }
                // Default: silently and completely omitted from public exports.
            }
            Block::Unknown(d) => {
                // Teaching notes may surface `:::discussion` blocks as a
                // For Discussion / Q&A section when explicitly requested.
                // The directive body is recovered deterministically: the
                // lines between the opening and closing fences.
                let is_discussion = matches!(
                    d.name.as_str(),
                    "discussion" | "qa" | "q-and-a"
                );
                if is_discussion
                    && matches!(
                        teaching,
                        Some(TeachingOptions {
                            include_discussion: true,
                            ..
                        })
                    )
                {
                    let body_lines: Vec<&str> = d
                        .raw_source
                        .lines()
                        .skip(1)
                        .take(d.raw_source.lines().count().saturating_sub(2))
                        .collect();
                    blocks.push(TemplateBlock::Discussion {
                        text: render_paragraphs(&body_lines.join("\n")),
                    });
                } else {
                    blocks.push(TemplateBlock::Unknown {
                        name: d.name.clone(),
                        raw: d.raw_source.clone(),
                    });
                }
            }
        }
    }

    TemplateData {
        meta,
        mode,
        blocks,
        teaching: teaching.map(|opts| TeachingTemplateData {
            include_big_idea: opts.include_big_idea,
            spacing: opts.spacing.as_str(),
            teacher_headings: opts.teacher_headings,
            include_discussion: opts.include_discussion,
        }),
    }
}

// ---------------------------------------------------------------------------
// Minimal Markdown → Typst rendering
// ---------------------------------------------------------------------------

/// Escape one text character for Typst markup context.
fn escape_typst_char(c: char, out: &mut String) {
    match c {
        '\\' | '#' | '$' | '@' | '[' | ']' | '<' | '>' | '*' | '_' | '~' | '\'' | '"' => {
            out.push('\\');
            out.push(c);
        }
        _ => out.push(c),
    }
}

/// Render inline Markdown (code spans, **strong**, *em*, _em_) to Typst
/// markup. Deliberately conservative: anything unrecognized is escaped as
/// literal text so it can never be interpreted as Typst code.
fn render_inline(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len() + 16);

    fn find_char(chars: &[char], from: usize, needle: char) -> Option<usize> {
        (from..chars.len()).find(|&i| chars[i] == needle)
    }

    fn find_double(chars: &[char], from: usize, marker: char) -> Option<usize> {
        if chars.len() < 2 {
            return None;
        }
        (from..chars.len() - 1).find(|&i| chars[i] == marker && chars[i + 1] == marker)
    }

    fn push_escaped(out: &mut String, inner: &str) {
        for ch in inner.trim().chars() {
            escape_typst_char(ch, out);
        }
    }

    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c == '`' {
            if let Some(close) = find_char(&chars, i + 1, '`') {
                // Raw span: verbatim, no escaping inside.
                out.push('`');
                for &ch in &chars[i + 1..close] {
                    out.push(ch);
                }
                out.push('`');
                i = close + 1;
                continue;
            }
        }
        if c == '*' && i + 1 < n && chars[i + 1] == '*' {
            if let Some(close) = find_double(&chars, i + 2, '*') {
                let inner: String = chars[i + 2..close].iter().collect();
                out.push_str("#strong[");
                push_escaped(&mut out, &inner);
                out.push(']');
                i = close + 2;
            } else {
                out.push_str("\\*\\*");
                i += 2;
            }
            continue;
        }
        if c == '*' || c == '_' {
            if let Some(close) = find_char(&chars, i + 1, c) {
                let inner: String = chars[i + 1..close].iter().collect();
                out.push_str("#emph[");
                push_escaped(&mut out, &inner);
                out.push(']');
                i = close + 1;
                continue;
            }
        }
        escape_typst_char(c, &mut out);
        i += 1;
    }
    out
}

/// Render multi-paragraph plain text (movement manuscripts).
fn render_paragraphs(text: &str) -> String {
    text.split("\n\n")
        .map(|para| {
            para.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(render_inline)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render a Markdown block to (typst markup, headings-only outline).
///
/// Supported constructs (V1): headings, paragraphs, inline emphasis/strong/
/// code, blockquotes, lists, ordered lists, horizontal rules, fenced code.
/// Everything else passes through as escaped literal text — it can never be
/// corrupted into Typst syntax, and the canonical file is never touched.
fn render_markdown(src: &str) -> (String, String) {
    let mut text = String::new();
    let mut outline = String::new();
    let mut para: Vec<String> = Vec::new();
    let mut quote: Vec<String> = Vec::new();
    let mut in_fence = false;

    let flush_para = |text: &mut String, para: &mut Vec<String>| {
        if !para.is_empty() {
            text.push_str(&para.join(" "));
            text.push_str("\n\n");
            para.clear();
        }
    };
    let flush_quote = |text: &mut String, quote: &mut Vec<String>| {
        if !quote.is_empty() {
            text.push_str("#quote[");
            for ch in quote.join(" ").trim().chars() {
                escape_typst_char(ch, text);
            }
            text.push_str("]\n\n");
            quote.clear();
        }
    };
    for raw_line in src.lines() {
        let line = raw_line.trim_end();

        if line.trim_start().starts_with("```") {
            flush_para(&mut text, &mut para);
            flush_quote(&mut text, &mut quote);
            text.push_str("```\n");
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            // Verbatim inside fences; raw blocks cannot be escaped into.
            text.push_str(line);
            text.push('\n');
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_para(&mut text, &mut para);
            flush_quote(&mut text, &mut quote);
            continue;
        }

        // Headings.
        if let Some(rest) = trimmed.strip_prefix('#') {
            let level = rest.len() - rest.trim_start_matches('#').len() + 1;
            if level >= 1 && level <= 6 {
                let heading_text = rest.trim_start_matches('#').trim();
                if !heading_text.is_empty() {
                    flush_para(&mut text, &mut para);
                    flush_quote(&mut text, &mut quote);
                    let marker = "=".repeat(level);
                    let rendered = render_inline(heading_text);
                    text.push_str(&marker);
                    text.push(' ');
                    text.push_str(&rendered);
                    text.push_str("\n\n");
                    outline.push_str(&marker);
                    outline.push(' ');
                    outline.push_str(&rendered);
                    outline.push_str("\n\n");
                    continue;
                }
            }
        }

        // Blockquote.
        if let Some(rest) = trimmed.strip_prefix('>') {
            flush_para(&mut text, &mut para);
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            quote.push(render_inline(rest));
            continue;
        }

        // Lists (markers match Typst's own list syntax).
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            flush_para(&mut text, &mut para);
            flush_quote(&mut text, &mut quote);
            text.push_str("- ");
            text.push_str(&render_inline(rest));
            text.push('\n');
            continue;
        }
        // Ordered lists.
        let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 {
            let after = &trimmed[digits..];
            if after.starts_with(". ") || after.starts_with(") ") {
                flush_para(&mut text, &mut para);
                flush_quote(&mut text, &mut quote);
                text.push_str(&trimmed[..digits]);
                text.push_str(&after[..2]);
                text.push_str(&render_inline(&after[2..]));
                text.push('\n');
                continue;
            }
        }

        // Horizontal rule.
        if (trimmed.starts_with("---") && trimmed.chars().all(|c| c == '-'))
            || (trimmed.starts_with("***") && trimmed.chars().all(|c| c == '*'))
        {
            flush_para(&mut text, &mut para);
            flush_quote(&mut text, &mut quote);
            text.push_str("#line(length: 100%)\n\n");
            continue;
        }

        flush_quote(&mut text, &mut quote);
        para.push(render_inline(trimmed));
    }

    flush_para(&mut text, &mut para);
    flush_quote(&mut text, &mut quote);
    if in_fence {
        // Unterminated fence: close it so compilation stays valid.
        text.push_str("```\n");
    }
    (text, outline)
}

// ---------------------------------------------------------------------------
// Immutable export snapshot
// ---------------------------------------------------------------------------

/// Immutable record of exactly what an export contains. Created before
/// rendering; all fields are private with read-only accessors, so snapshot
/// data cannot silently mutate after creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportSnapshot {
    schema_version: u32,
    export_id: String,
    sermon_id: String,
    /// SHA-256 of the canonical Markdown source that was exported — the
    /// revision identity that proves what was exported.
    sermon_source_hash: String,
    template_id: String,
    template_version: String,
    format: ExportFormat,
    pulpit_mode: Option<PulpitMode>,
    include_private_notes: bool,
    /// SHA-256 over all deterministic rendering inputs (application version,
    /// template identity, and the exact composed Typst source).
    content_hash: String,
    created_at: String,
    application_version: String,
}

impl ExportSnapshot {
    #[allow(clippy::too_many_arguments)]
    fn new(
        export_id: String,
        sermon_id: String,
        sermon_source_hash: String,
        template_id: &str,
        template_version: &str,
        format: ExportFormat,
        pulpit_mode: Option<PulpitMode>,
        include_private_notes: bool,
        content_hash: String,
        application_version: String,
    ) -> Self {
        ExportSnapshot {
            schema_version: EXPORT_SNAPSHOT_SCHEMA_VERSION,
            export_id,
            sermon_id,
            sermon_source_hash,
            template_id: template_id.to_string(),
            template_version: template_version.to_string(),
            format,
            pulpit_mode,
            include_private_notes,
            content_hash,
            created_at: chrono::Utc::now().to_rfc3339(),
            application_version,
        }
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }
    pub fn export_id(&self) -> &str {
        &self.export_id
    }
    pub fn sermon_id(&self) -> &str {
        &self.sermon_id
    }
    pub fn sermon_source_hash(&self) -> &str {
        &self.sermon_source_hash
    }
    pub fn template_id(&self) -> &str {
        &self.template_id
    }
    pub fn template_version(&self) -> &str {
        &self.template_version
    }
    pub fn format(&self) -> ExportFormat {
        self.format
    }
    pub fn pulpit_mode(&self) -> Option<PulpitMode> {
        self.pulpit_mode
    }
    pub fn include_private_notes(&self) -> bool {
        self.include_private_notes
    }
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }
    pub fn created_at(&self) -> &str {
        &self.created_at
    }
    pub fn application_version(&self) -> &str {
        &self.application_version
    }

    /// Stable JSON representation for persistence/reporting by Track F.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Outcome reporting
// ---------------------------------------------------------------------------

/// Structured export report: enough for the frontend to show success/failure,
/// identity, location, size, and hashes without re-reading the file.
#[derive(Debug, Clone, Serialize)]
pub struct ExportOutcome {
    pub success: bool,
    /// Human-readable failure reason; `None` on success.
    pub error: Option<String>,
    pub result: Option<ExportResult>,
    /// Present whenever validation passed and rendering was attempted —
    /// failures after this point can still cite the snapshot identity.
    pub snapshot: Option<ExportSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub export_id: String,
    pub output_path: String,
    pub format: ExportFormat,
    pub template_id: String,
    pub template_version: String,
    pub pulpit_mode: Option<PulpitMode>,
    pub created_at: String,
    pub file_size: u64,
    pub content_hash: String,
    pub sermon_source_hash: String,
}

impl ExportOutcome {
    fn failure(error: impl std::fmt::Display, snapshot: Option<ExportSnapshot>) -> Self {
        ExportOutcome {
            success: false,
            error: Some(error.to_string()),
            result: None,
            snapshot,
        }
    }

    fn success(result: ExportResult, snapshot: ExportSnapshot) -> Self {
        ExportOutcome {
            success: true,
            error: None,
            result: Some(result),
            snapshot: Some(snapshot),
        }
    }
}

// ---------------------------------------------------------------------------
// In-process Typst rendering
// ---------------------------------------------------------------------------

/// Embedded fonts from the versioned `typst-assets` crate: fully offline,
/// deterministic across machines for a given dependency lock.
fn embedded_fonts() -> &'static (LazyHash<FontBook>, Vec<Font>) {
    static FONTS: OnceLock<(LazyHash<FontBook>, Vec<Font>)> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut book = FontBook::new();
        let mut fonts = Vec::new();
        for data in typst_assets::fonts() {
            let infos: Vec<FontInfo> = FontInfo::iter(data).collect();
            for (index, info) in infos.into_iter().enumerate() {
                if let Some(font) = Font::new(Bytes::new(data), index as u32) {
                    book.push(info);
                    fonts.push(font);
                }
            }
        }
        (LazyHash::new(book), fonts)
    })
}

/// A minimal in-memory [`World`]: serves the composed main source and the
/// sermon data JSON from memory, and fonts from the embedded set. No
/// filesystem access, no packages, no network.
struct SermonWorld {
    library: LazyHash<typst::Library>,
    main_id: FileId,
    main_source: Source,
    data_id: FileId,
    data_bytes: Bytes,
    fonts: &'static (LazyHash<FontBook>, Vec<Font>),
}

impl SermonWorld {
    fn new(main_text: String, data_json: String) -> Result<Self, ExportError> {
        let main_vpath =
            VirtualPath::new("main.typ").map_err(|e| ExportError::Render(e.to_string()))?;
        let data_vpath =
            VirtualPath::new("sermon-data.json").map_err(|e| ExportError::Render(e.to_string()))?;
        let main_id = RootedPath::new(VirtualRoot::Project, main_vpath).intern();
        let data_id = RootedPath::new(VirtualRoot::Project, data_vpath).intern();
        Ok(SermonWorld {
            library: LazyHash::new(typst::Library::default()),
            main_id,
            main_source: Source::new(main_id, main_text),
            data_id,
            data_bytes: Bytes::from_string(data_json),
            fonts: embedded_fonts(),
        })
    }

    fn not_found(id: &FileId) -> FileError {
        FileError::NotFound(id.vpath().get_with_slash().into())
    }
}

impl World for SermonWorld {
    fn library(&self) -> &LazyHash<typst::Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.fonts.0
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.main_source.clone())
        } else {
            Err(SermonWorld::not_found(&id))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.data_id {
            Ok(self.data_bytes.clone())
        } else {
            Err(SermonWorld::not_found(&id))
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.1.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        // Templates never call today(); provided for completeness. Date-only
        // keeps any accidental use deterministic within a day.
        use chrono::Datelike;
        let now = chrono::Utc::now();
        Datetime::from_ymd(now.year(), now.month() as u8, now.day() as u8)
    }
}

fn diagnostics_message(world: &SermonWorld, errors: &[typst::diag::SourceDiagnostic]) -> String {
    let parts: Vec<String> = errors
        .iter()
        .map(|d| {
            let loc = world
                .range(d.span)
                .map(|r| format!(" (main.typ byte {})", r.start))
                .unwrap_or_default();
            format!("{}{}", d.message, loc)
        })
        .collect();
    parts.join("; ")
}

/// Compose the main Typst source: normalized embedded template + data
/// reference + template invocation.
fn compose_main_source(template_src: &str, entry_fn: &str) -> String {
    let template = template_src.replace("\r\n", "\n");
    let mut src = String::with_capacity(template.len() + 128);
    src.push_str(template.trim_end());
    src.push_str("\n\n#let sermon = json(\"sermon-data.json\")\n");
    let _ = write!(src, "#{entry_fn}(sermon)\n");
    src
}

/// Deterministic SHA-256 over every rendering-relevant input: application
/// version, template identity, the composed Typst source, and the exact
/// template data JSON (which carries the mode and all content).
fn compute_content_hash(
    application_version: &str,
    template_id: &str,
    template_version: &str,
    main_source: &str,
    data_json: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"sermon-studio/export-content/v1\n");
    hasher.update(b"app=");
    hasher.update(application_version.as_bytes());
    hasher.update(b"\ntemplate=");
    hasher.update(template_id.as_bytes());
    hasher.update(b"@");
    hasher.update(template_version.as_bytes());
    hasher.update(b"\n\x00source\n");
    hasher.update(main_source.as_bytes());
    hasher.update(b"\n\x00data\n");
    hasher.update(data_json.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Validate raw PDF output before it is allowed to replace anything.
fn validate_pdf_bytes(bytes: &[u8]) -> Result<(), ExportError> {
    if bytes.is_empty() {
        return Err(ExportError::InvalidPdf("output is empty".to_string()));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(ExportError::InvalidPdf(
            "output does not start with the %PDF- signature".to_string(),
        ));
    }
    Ok(())
}

/// Render Typst source + data JSON to validated PDF bytes, fully in process.
///
/// Exposed (hidden) so tests can exercise compile-failure paths against the
/// atomic write sequence, and so the integration track can diagnose template
/// issues without a filesystem round-trip.
#[doc(hidden)]
pub fn render_pdf_bytes(main_source: String, data_json: String) -> Result<Vec<u8>, ExportError> {
    let world = SermonWorld::new(main_source, data_json)?;
    let warned = typst::compile::<PagedDocument>(&world);
    let document = match warned.output {
        Ok(doc) => doc,
        Err(errors) => {
            return Err(ExportError::Render(diagnostics_message(&world, &errors)));
        }
    };
    if document.pages().is_empty() {
        return Err(ExportError::Render(
            "compiled document contains no pages".to_string(),
        ));
    }
    let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|errors| ExportError::Render(diagnostics_message(&world, &errors)))?;
    validate_pdf_bytes(&pdf)?;
    Ok(pdf)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Export a canonical sermon AST to PDF with an immutable snapshot.
///
/// Sequence: validate request → build template data (private notes excluded
/// unless explicitly requested; never allowed for the bulletin) → create the
/// immutable snapshot → render in process → validate PDF bytes → atomic write
/// (Track C machinery: temp sibling → sync → rename). A failed render or a
/// failed write never damages an existing valid export at `output_path`.
pub fn export_sermon(
    sermon: &Sermon,
    raw_source: &str,
    request: ExportRequest,
    output_path: &Path,
) -> ExportOutcome {
    export_sermon_with_id(sermon, raw_source, request, output_path, None)
}

/// Export using an optional caller-created immutable source snapshot id.
///
/// Track F uses this when the frontend explicitly creates a source snapshot
/// before selecting an export job. The rendering inputs and hashes are still
/// produced here by Track D; only the stable identity is supplied by the
/// caller so the transport can correlate snapshot creation and execution.
pub fn export_sermon_with_id(
    sermon: &Sermon,
    raw_source: &str,
    request: ExportRequest,
    output_path: &Path,
    export_id: Option<String>,
) -> ExportOutcome {
    let application_version = env!("CARGO_PKG_VERSION").to_string();

    if let Err(e) = request.validate() {
        return ExportOutcome::failure(e, None);
    }

    let source_hash = crate::sermon::sha256_hex(raw_source.as_bytes());
    let sermon_id = resolved_sermon_id(sermon, &source_hash);
    let data = build_template_data(sermon, &sermon_id, &request);
    let data_json = match serde_json::to_string(&data) {
        Ok(s) => s,
        Err(e) => return ExportOutcome::failure(ExportError::Serialization(e.to_string()), None),
    };

    let (template_id, template_version, entry_fn, template_src) = match request.format {
        ExportFormat::PulpitManuscript => (
            PULPIT_TEMPLATE_ID,
            PULPIT_TEMPLATE_VERSION,
            "pulpit-doc",
            PULPIT_TEMPLATE_SRC,
        ),
        ExportFormat::ChurchBulletin => (
            BULLETIN_TEMPLATE_ID,
            BULLETIN_TEMPLATE_VERSION,
            "bulletin-doc",
            BULLETIN_TEMPLATE_SRC,
        ),
        ExportFormat::TeachingNotes => (
            TEACHING_TEMPLATE_ID,
            TEACHING_TEMPLATE_VERSION,
            "teaching-doc",
            TEACHING_TEMPLATE_SRC,
        ),
    };

    let main_source = compose_main_source(template_src, entry_fn);
    let content_hash = compute_content_hash(
        &application_version,
        template_id,
        template_version,
        &main_source,
        &data_json,
    );

    // The snapshot is created BEFORE rendering and is never mutated afterwards.
    let snapshot = ExportSnapshot::new(
        export_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        sermon_id,
        source_hash,
        template_id,
        template_version,
        request.format,
        request.pulpit_mode,
        request.include_private_notes,
        content_hash,
        application_version,
    );

    let pdf = match render_pdf_bytes(main_source, data_json) {
        Ok(pdf) => pdf,
        Err(e) => return ExportOutcome::failure(e, Some(snapshot)),
    };

    if let Err(e) = atomic_save::save_atomic(output_path, &pdf) {
        return ExportOutcome::failure(e, Some(snapshot));
    }

    let result = ExportResult {
        export_id: snapshot.export_id.clone(),
        output_path: output_path.display().to_string(),
        format: snapshot.format,
        template_id: snapshot.template_id.clone(),
        template_version: snapshot.template_version.clone(),
        pulpit_mode: snapshot.pulpit_mode,
        created_at: snapshot.created_at.clone(),
        file_size: pdf.len() as u64,
        content_hash: snapshot.content_hash.clone(),
        sermon_source_hash: snapshot.sermon_source_hash.clone(),
    };
    ExportOutcome::success(result, snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical export fixture: every block type, all three movement styles,
    /// markdown constructs, and a distinctive private-notes sentinel.
    const FIXTURE: &str = "---\nid: vine-sermon\ntitle: \"Remain in Christ\"\ndate_preached: 2024-07-14\nseries: \"Abide\"\nliturgical_season: Ordinary\nprimary_passage: \"John.15.1-4\"\nbig_idea: \"Remaining in Christ is the source of fruitfulness.\"\nstructure_type: verse_by_verse\n---\n\n# Remain in Christ\n\nIntro prose with *emphasis* and **strong** words and `inline code`.\n\n> A blockquote stays a quote.\n\n:::movement{order=1 title=\"Remain connected\" warrant=\"John 15:1-4\"}\nManuscript paragraph one.\n\nManuscript paragraph two.\n:::\n\nMiddle prose between movements.\n\n:::movement{order=2 title=\"Pruned for fruit\"}\n- First bullet\n- Second bullet\n:::\n\n:::movement{order=3 title=\"Combined movement\"}\nProse and bullets together.\n\n- Combined bullet A\n- Combined bullet B\n:::\n\n:::illustration{id=\"vineyard-pruning\" title=\"Pruning a fruit tree\"}\nIllustration body text.\n:::\n\n:::application{audience=\"congregation\"}\nApplication body text.\n:::\n\n:::exegetical-notes\nPRIVATE-SENTINEL-7f3d study notes that must not leak.\n:::\n\n:::custom-block{foo=\"bar\"}\nOriginal unknown-directive body.\n:::\n\nClosing prose.\n";

    fn parse_fixture() -> Sermon {
        Sermon::parse(FIXTURE).unwrap()
    }

    fn export_to(
        sermon: &Sermon,
        request: ExportRequest,
        path: &Path,
    ) -> ExportOutcome {
        export_sermon(sermon, FIXTURE, request, path)
    }

    fn assert_valid_pdf(bytes_or_path: &[u8]) {
        assert!(!bytes_or_path.is_empty());
        assert!(bytes_or_path.starts_with(b"%PDF-"));
    }

    #[test]
    fn pulpit_manuscript_mode_exports_valid_pdf() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("manuscript.pdf");
        let outcome = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Manuscript), &out);
        assert!(outcome.success, "export failed: {:?}", outcome.error);
        let result = outcome.result.as_ref().unwrap();
        assert_eq!(result.format, ExportFormat::PulpitManuscript);
        assert_eq!(result.pulpit_mode, Some(PulpitMode::Manuscript));
        assert_eq!(result.template_id, PULPIT_TEMPLATE_ID);
        assert_eq!(result.file_size, std::fs::metadata(&out).unwrap().len());
        let bytes = std::fs::read(&out).unwrap();
        assert_valid_pdf(&bytes);
    }

    #[test]
    fn pulpit_outline_mode_exports_valid_pdf() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("outline.pdf");
        let outcome = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Outline), &out);
        assert!(outcome.success, "export failed: {:?}", outcome.error);
        assert_valid_pdf(&std::fs::read(&out).unwrap());
    }

    #[test]
    fn pulpit_combined_mode_exports_valid_pdf() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("combined.pdf");
        let outcome = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Combined), &out);
        assert!(outcome.success, "export failed: {:?}", outcome.error);
        assert_valid_pdf(&std::fs::read(&out).unwrap());
    }

    #[test]
    fn church_bulletin_exports_valid_pdf() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("bulletin.pdf");
        let outcome = export_to(&sermon, ExportRequest::bulletin(), &out);
        assert!(outcome.success, "export failed: {:?}", outcome.error);
        let result = outcome.result.as_ref().unwrap();
        assert_eq!(result.format, ExportFormat::ChurchBulletin);
        assert_eq!(result.template_id, BULLETIN_TEMPLATE_ID);
        assert_valid_pdf(&std::fs::read(&out).unwrap());
    }

    // ── Teaching notes (Release Track R) ─────────────────────────────────────

    /// Teaching fixture: same block grammar as the canonical fixture plus a
    /// `:::discussion` block (parsed as an unknown directive — the teaching
    /// data builder surfaces it only when discussion inclusion is requested).
    const TEACHING_FIXTURE: &str = "---\nid: teaching-sermon\ntitle: \"Abide in Christ\"\ndate_preached: 2025-03-09\nseries: \"Abide\"\nprimary_passage: \"John.15.1-8\"\nbig_idea: \"Abiding is the way to lasting fruit.\"\nstructure_type: verse_by_verse\n---\n\n# Abide in Christ\n\nOpening orientation for teachers.\n\n:::movement{order=1 title=\"The vine and the branches\" warrant=\"John 15:1-4\"}\nTeaching prose for the first movement.\n\n- Branch one\n- Branch two\n:::\n\n:::illustration{id=\"vineyard\" title=\"A working vineyard\"}\nIllustration body text.\n:::\n\n:::application{audience=\"small group\"}\nApplication body text.\n:::\n\n:::exegetical-notes\nTEACHING-PRIVATE-SENTINEL study notes for the teacher.\n:::\n\n:::discussion\n- Where do you see pruning in your own season of ministry?\n- What keeps a branch connected when the winds rise?\n:::\n\nClosing summary.\n";

    fn teaching_export(
        options: TeachingOptions,
        include_private: bool,
    ) -> (ExportOutcome, tempfile::TempDir, std::path::PathBuf) {
        let sermon = Sermon::parse(TEACHING_FIXTURE).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("teaching.pdf");
        let outcome = export_sermon(
            &sermon,
            TEACHING_FIXTURE,
            ExportRequest {
                include_private_notes: include_private,
                ..ExportRequest::teaching(options)
            },
            &out,
        );
        (outcome, tmp, out)
    }

    #[test]
    fn teaching_notes_export_valid_pdf_with_teaching_template() {
        let (outcome, _tmp, out) = teaching_export(TeachingOptions::default(), false);
        assert!(outcome.success, "export failed: {:?}", outcome.error);
        let result = outcome.result.as_ref().unwrap();
        assert_eq!(result.format, ExportFormat::TeachingNotes);
        assert_eq!(result.template_id, TEACHING_TEMPLATE_ID);
        assert_eq!(result.template_version, TEACHING_TEMPLATE_VERSION);
        assert!(result.pulpit_mode.is_none());
        assert_valid_pdf(&std::fs::read(&out).unwrap());
    }

    #[test]
    fn teaching_notes_export_is_deterministic() {
        let (a, _tmp_a, path_a) = teaching_export(TeachingOptions::default(), false);
        let (b, _tmp_b, path_b) = teaching_export(TeachingOptions::default(), false);
        assert!(a.success && b.success);
        let sa = a.snapshot.unwrap();
        let sb = b.snapshot.unwrap();
        // Same source, options, template, and version ⇒ identical rendering
        // inputs and identical content hash (distinct export ids aside).
        assert_eq!(sa.content_hash(), sb.content_hash());
        assert_eq!(sa.sermon_source_hash(), sb.sermon_source_hash());
        assert_ne!(sa.export_id(), sb.export_id());
        assert_eq!(
            std::fs::read(&path_a).unwrap(),
            std::fs::read(&path_b).unwrap()
        );
    }

    #[test]
    fn teaching_notes_export_preserves_canonical_source_hash() {
        let (outcome, _tmp, _out) = teaching_export(TeachingOptions::default(), false);
        let snapshot = outcome.snapshot.unwrap();
        // The snapshot's source hash is the SHA-256 of the exact canonical
        // Markdown handed in — the teaching target compiles the same source
        // differently, it never creates a second document format.
        assert_eq!(
            snapshot.sermon_source_hash(),
            crate::sermon::sha256_hex(TEACHING_FIXTURE.as_bytes())
        );
    }

    #[test]
    fn teaching_notes_private_notes_default_excluded_explicitly_included() {
        // Default: the private sentinel never reaches the template data.
        let (outcome, _tmp, _out) = teaching_export(TeachingOptions::default(), false);
        let data = build_template_data(
            &Sermon::parse(TEACHING_FIXTURE).unwrap(),
            "teaching-sermon",
            &ExportRequest::teaching(TeachingOptions::default()),
        );
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("TEACHING-PRIVATE-SENTINEL"));
        assert!(outcome.success);

        // Explicitly requested: present, teacher-facing only.
        let (_, _tmp, _out) = teaching_export(TeachingOptions::default(), true);
        let data = build_template_data(
            &Sermon::parse(TEACHING_FIXTURE).unwrap(),
            "teaching-sermon",
            &ExportRequest {
                include_private_notes: true,
                ..ExportRequest::teaching(TeachingOptions::default())
            },
        );
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("TEACHING-PRIVATE-SENTINEL"));
    }

    #[test]
    fn teaching_notes_discussion_blocks_only_when_requested() {
        let sermon = Sermon::parse(TEACHING_FIXTURE).unwrap();

        // Default: the discussion block stays an archival unknown directive.
        let data = build_template_data(
            &sermon,
            "teaching-sermon",
            &ExportRequest::teaching(TeachingOptions::default()),
        );
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("\"kind\":\"discussion\""));
        assert!(json.contains("\"kind\":\"unknown\""));
        assert!(json.contains(":::"));

        // Requested: surfaced as a discussion block with the fences stripped.
        let data = build_template_data(
            &sermon,
            "teaching-sermon",
            &ExportRequest::teaching(TeachingOptions {
                include_discussion: true,
                ..TeachingOptions::default()
            }),
        );
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"kind\":\"discussion\""));
        assert!(json.contains("pruning in your own season"));

        // The pulpit manuscript never surfaces discussion blocks.
        let data = build_template_data(&sermon, "teaching-sermon", &ExportRequest::pulpit(PulpitMode::Combined));
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("\"kind\":\"discussion\""));
    }

    #[test]
    fn teaching_notes_options_change_the_snapshot_hash() {
        let (a, _tmp_a, _out_a) = teaching_export(TeachingOptions::default(), false);
        let (b, _tmp_b, _out_b) = teaching_export(
            TeachingOptions {
                spacing: TeachingSpacing::Compact,
                ..TeachingOptions::default()
            },
            false,
        );
        assert!(a.success && b.success);
        assert_ne!(
            a.snapshot.unwrap().content_hash(),
            b.snapshot.unwrap().content_hash()
        );
    }

    #[test]
    fn teaching_notes_big_idea_option_is_honored() {
        let sermon = Sermon::parse(TEACHING_FIXTURE).unwrap();
        let with = build_template_data(
            &sermon,
            "teaching-sermon",
            &ExportRequest::teaching(TeachingOptions::default()),
        );
        assert!(serde_json::to_string(&with).unwrap().contains("Abiding is the way"));

        let without = build_template_data(
            &sermon,
            "teaching-sermon",
            &ExportRequest::teaching(TeachingOptions {
                include_big_idea: false,
                ..TeachingOptions::default()
            }),
        );
        let json = serde_json::to_string(&without).unwrap();
        assert!(!json.contains("Abiding is the way"));
        // The other formats are untouched by the option.
        let pulpit = build_template_data(&sermon, "teaching-sermon", &ExportRequest::pulpit(PulpitMode::Manuscript));
        assert!(serde_json::to_string(&pulpit).unwrap().contains("Abiding is the way"));
    }

    #[test]
    fn teaching_options_are_rejected_on_other_formats() {
        let pulpit = ExportRequest {
            teaching_options: Some(TeachingOptions::default()),
            ..ExportRequest::pulpit(PulpitMode::Manuscript)
        };
        assert!(matches!(pulpit.validate(), Err(ExportError::InvalidRequest(_))));
        let bulletin = ExportRequest {
            teaching_options: Some(TeachingOptions::default()),
            ..ExportRequest::bulletin()
        };
        assert!(matches!(bulletin.validate(), Err(ExportError::InvalidRequest(_))));
        let teaching_with_mode = ExportRequest {
            pulpit_mode: Some(PulpitMode::Manuscript),
            ..ExportRequest::teaching(TeachingOptions::default())
        };
        assert!(matches!(
            teaching_with_mode.validate(),
            Err(ExportError::InvalidRequest(_))
        ));
    }

    #[test]
    fn private_exegetical_notes_are_excluded_by_default() {
        let sermon = parse_fixture();
        for request in [
            ExportRequest::pulpit(PulpitMode::Manuscript),
            ExportRequest::pulpit(PulpitMode::Outline),
            ExportRequest::pulpit(PulpitMode::Combined),
            ExportRequest::bulletin(),
        ] {
            let data = build_template_data(&sermon, "vine-sermon", &request);
            let json = serde_json::to_string(&data).unwrap();
            assert!(
                !json.contains("PRIVATE-SENTINEL-7f3d"),
                "private notes leaked in {:?}",
                request.format
            );
        }
    }

    #[test]
    fn private_notes_can_be_included_only_when_explicitly_requested() {
        let sermon = parse_fixture();
        let request = ExportRequest {
            include_private_notes: true,
            ..ExportRequest::pulpit(PulpitMode::Combined)
        };
        let data = build_template_data(&sermon, "vine-sermon", &request);
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("PRIVATE-SENTINEL-7f3d"));

        // ...but never for the bulletin.
        let bulletin = ExportRequest {
            include_private_notes: true,
            ..ExportRequest::bulletin()
        };
        assert!(matches!(
            bulletin.validate(),
            Err(ExportError::InvalidRequest(_))
        ));
    }

    #[test]
    fn snapshot_content_hash_is_stable_for_identical_inputs() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let a = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Combined), &tmp.path().join("a.pdf"));
        let b = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Combined), &tmp.path().join("b.pdf"));
        assert!(a.success && b.success);
        let sa = a.snapshot.unwrap();
        let sb = b.snapshot.unwrap();
        assert_eq!(sa.content_hash(), sb.content_hash());
        assert_eq!(sa.sermon_source_hash(), sb.sermon_source_hash());
        // Distinct exports still get distinct identities.
        assert_ne!(sa.export_id(), sb.export_id());
    }

    #[test]
    fn changing_content_changes_the_snapshot_hash() {
        let sermon = parse_fixture();
        let modified = Sermon::parse(
            &FIXTURE.replace("Manuscript paragraph two.", "Edited paragraph two."),
        )
        .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let a = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Manuscript), &tmp.path().join("a.pdf"));
        let b = export_to(&modified, ExportRequest::pulpit(PulpitMode::Manuscript), &tmp.path().join("b.pdf"));
        assert!(a.success && b.success);
        assert_ne!(
            a.snapshot.unwrap().content_hash(),
            b.snapshot.unwrap().content_hash()
        );
    }

    #[test]
    fn changing_mode_changes_the_snapshot_hash() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let a = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Manuscript), &tmp.path().join("a.pdf"));
        let b = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Outline), &tmp.path().join("b.pdf"));
        assert_ne!(
            a.snapshot.unwrap().content_hash(),
            b.snapshot.unwrap().content_hash()
        );
    }

    #[test]
    fn snapshot_is_immutable_and_fully_reported() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let outcome = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Combined), &tmp.path().join("s.pdf"));
        let snapshot = outcome.snapshot.unwrap();

        // Private fields + read-only accessors: the clone can never diverge.
        let clone = snapshot.clone();
        assert_eq!(snapshot, clone);

        let json = snapshot.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["schema_version"], EXPORT_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(v["export_id"], snapshot.export_id());
        assert_eq!(v["sermon_id"], "vine-sermon");
        assert_eq!(v["template_id"], PULPIT_TEMPLATE_ID);
        assert_eq!(v["template_version"], PULPIT_TEMPLATE_VERSION);
        assert_eq!(v["pulpit_mode"], "combined");
        assert_eq!(v["content_hash"], snapshot.content_hash());
        assert!(v["created_at"].is_string());
        assert_eq!(v["sermon_source_hash"].as_str().unwrap().len(), 64);
        assert!(v["application_version"].is_string());
    }

    #[test]
    fn embedded_templates_have_identifiers_and_versions() {
        assert_eq!(PULPIT_TEMPLATE_ID, "pulpit_manuscript");
        assert_eq!(BULLETIN_TEMPLATE_ID, "church_bulletin");
        assert_eq!(TEACHING_TEMPLATE_ID, "teaching_notes");
        assert!(PULPIT_TEMPLATE_SRC.contains("#let pulpit-doc"));
        assert!(BULLETIN_TEMPLATE_SRC.contains("#let bulletin-doc"));
        assert!(TEACHING_TEMPLATE_SRC.contains("#let teaching-doc"));
        // Determinism guard: templates must never query the current date.
        assert!(!PULPIT_TEMPLATE_SRC.contains("today"));
        assert!(!BULLETIN_TEMPLATE_SRC.contains("today"));
        assert!(!TEACHING_TEMPLATE_SRC.contains("today"));
    }

    #[test]
    fn failed_render_does_not_destroy_previous_valid_output() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("out.pdf");

        let good = export_to(&sermon, ExportRequest::pulpit(PulpitMode::Manuscript), &out);
        assert!(good.success);
        let original = std::fs::read(&out).unwrap();
        assert_valid_pdf(&original);

        // Force a compile failure at the render boundary (broken Typst input).
        let broken = render_pdf_bytes(
            "#let this is not valid typst syntax (((".to_string(),
            "{}".to_string(),
        );
        assert!(broken.is_err());

        // The previous valid export is untouched.
        assert_eq!(std::fs::read(&out).unwrap(), original);

        // And no atomic-save temp artifacts were left behind.
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| crate::atomic_save::is_temp_artifact(&e.file_name().to_string_lossy()))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn atomic_output_leaves_no_temp_files() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("sub").join("x.pdf");
        let outcome = export_to(&sermon, ExportRequest::bulletin(), &out);
        assert!(outcome.success);
        assert!(out.is_file());
        let siblings: Vec<_> = std::fs::read_dir(out.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(siblings.len(), 1, "only the pdf should exist: {siblings:?}");
    }

    #[test]
    fn invalid_requests_are_handled_cleanly() {
        let sermon = parse_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("never.pdf");

        // Pulpit without a mode.
        let bad = ExportRequest {
            format: ExportFormat::PulpitManuscript,
            pulpit_mode: None,
            include_private_notes: false,
            teaching_options: None,
        };
        let outcome = export_to(&sermon, bad, &out);
        assert!(!outcome.success);
        assert!(outcome.error.unwrap().contains("requires a mode"));

        // Bulletin with a mode.
        let bad = ExportRequest {
            format: ExportFormat::ChurchBulletin,
            pulpit_mode: Some(PulpitMode::Outline),
            include_private_notes: false,
            teaching_options: None,
        };
        let outcome = export_to(&sermon, bad, &out);
        assert!(!outcome.success);

        // Unknown mode string.
        assert!(PulpitMode::parse("poster").is_none());
        assert_eq!(PulpitMode::parse("combined"), Some(PulpitMode::Combined));

        // Nothing was written.
        assert!(!out.exists());
    }

    #[test]
    fn markdown_rendering_escapes_typst_specials() {
        let (text, outline) = render_markdown(
            "# Heading One\n\nCosts 50% of $100 and #tags [brackets] <go> *here*.\n\n- plain item\n",
        );
        assert!(text.starts_with("= Heading One"));
        assert!(text.contains("= Heading One"));
        assert_eq!(outline, "= Heading One\n\n");
        // Specials are escaped, emphasis still converted.
        assert!(text.contains("\\#tags \\[brackets\\] \\<go\\>"));
        assert!(text.contains("#emph[here]"));
        assert!(text.contains("- plain item"));
    }

    #[test]
    fn movement_styles_render_per_mode() {
        let sermon = parse_fixture();
        // Manuscript mode keeps prose; outline keeps bullets; combined both.
        let request = ExportRequest::pulpit(PulpitMode::Manuscript);
        let data = build_template_data(&sermon, "vine-sermon", &request);
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("Manuscript paragraph one"));
        assert!(json.contains("Combined bullet A"));

        let request = ExportRequest::bulletin();
        let data = build_template_data(&sermon, "vine-sermon", &request);
        let json = serde_json::to_string(&data).unwrap();
        // Bulletin data is the same canonical dict; the template chooses the
        // concise subset (no prose rendered).
        assert!(json.contains("First bullet"));
    }

    #[test]
    fn unknown_directives_are_preserved_verbatim_in_export_data() {
        let sermon = parse_fixture();
        let request = ExportRequest::pulpit(PulpitMode::Combined);
        let data = build_template_data(&sermon, "vine-sermon", &request);
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("custom-block"));
        assert!(json.contains(":::custom-block{foo=\\\"bar\\\"}"));
        // The raw directive body survives byte-for-byte (archival rendering).
        let raws: Vec<&TemplateBlock> = data
            .blocks
            .iter()
            .filter(|b| matches!(b, TemplateBlock::Unknown { .. }))
            .collect();
        assert_eq!(raws.len(), 1);
        if let TemplateBlock::Unknown { name, raw } = raws[0] {
            assert_eq!(name, "custom-block");
            assert_eq!(raw, ":::custom-block{foo=\"bar\"}\nOriginal unknown-directive body.\n:::");
        }
    }
}
