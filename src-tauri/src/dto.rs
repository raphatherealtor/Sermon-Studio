//! IPC transport DTOs and the mapping between the Rocket frontend contract
//! (`src/lib/backend/types.ts`) and the Wave 1 `sermon_core` types.
//!
//! Track F owns this module. Everything here is pure data transformation: no
//! I/O, no Tauri types — which keeps it fully unit-testable. The rules:
//!
//! * JSON is camelCase to match the TypeScript contract exactly.
//! * Reference transport preserves all three Track A resolution states
//!   (definite / ambiguous / invalid); nothing is collapsed to "success" and
//!   invalid references are never clamped.
//! * Fields the core genuinely does not track are filled with honest
//!   defaults (`""`, `0`, empty collections) — never fabricated data.

use serde::{Deserialize, Serialize};
use sermon_core::books;
use sermon_core::reference::{ParsedReference, Resolution};
use sermon_core::reconcile::FileState;

// ---------------------------------------------------------------------------
// Reference transport
// ---------------------------------------------------------------------------

/// The three explicit Track A resolution states, transported without loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolutionDto {
    Definite,
    Ambiguous,
    Invalid,
}

impl From<Resolution> for ResolutionDto {
    fn from(r: Resolution) -> Self {
        match r {
            Resolution::Definite => ResolutionDto::Definite,
            Resolution::Ambiguous => ResolutionDto::Ambiguous,
            Resolution::Invalid => ResolutionDto::Invalid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceMatchDto {
    /// Original matched source text, preserved verbatim.
    pub raw: String,
    /// OSIS-style book name (e.g. "John"); empty when no book was recognized.
    pub book: String,
    /// Chapter, or 0 when the reference is context-only (e.g. "v.6").
    pub chapter: i64,
    pub verse: Option<i64>,
    pub end_verse: Option<i64>,
    /// Byte offset of `raw` in the scanned text.
    pub offset: usize,
    /// Byte length of `raw`.
    pub length: usize,
    /// Canonical dotted form — present only for definite references.
    pub osis_id: Option<String>,
    /// Explicit Track A resolution state; never collapsed.
    pub resolution: ResolutionDto,
    /// Human-readable reason for ambiguous/invalid results.
    pub reason: Option<String>,
}

fn osis_name(book_num: i64) -> String {
    books::BOOKS
        .iter()
        .find(|b| b.0 == book_num)
        .map(|b| b.1.to_string())
        .unwrap_or_default()
}

impl From<ParsedReference> for ReferenceMatchDto {
    fn from(r: ParsedReference) -> Self {
        let verse = r.verse_start;
        let end_verse = match (r.verse_start, r.verse_end) {
            (Some(a), Some(b)) if b != a => Some(b),
            _ => None,
        };
        ReferenceMatchDto {
            raw: r.source.clone(),
            book: r.book_num.map(osis_name).unwrap_or_default(),
            chapter: r.chapter.unwrap_or(0),
            verse,
            end_verse,
            offset: 0,
            length: r.source.len(),
            osis_id: r.canonical(),
            resolution: r.resolution.into(),
            reason: r.reason,
        }
    }
}

/// Maximum words considered for one reference candidate. Longest match wins
/// at each start position, so "John 3:16-17" resolves as a range, not "John".
const MAX_REF_WORDS: usize = 6;

/// Scan prose for scripture references using Track A's `resolve()` as the
/// single authority. Candidate windows of 1..=6 words are tried longest
/// first at each position; a recognized span is skipped past. Ambiguous and
/// invalid references are reported exactly like definite ones.
pub fn scan_references(text: &str) -> Vec<ReferenceMatchDto> {
    // Word spans (byte offsets), punctuation included; resolve() trims.
    let mut words: Vec<(usize, usize)> = Vec::new();
    let mut rest = text;
    let mut base = 0usize;
    while let Some(off) = rest.find(|c: char| !c.is_whitespace()) {
        let start = base + off;
        let tail = &rest[off..];
        let end_rel = tail
            .find(|c: char| c.is_whitespace())
            .unwrap_or(tail.len());
        words.push((start, start + end_rel));
        base = start + end_rel;
        rest = &text[base..];
    }

    let mut out: Vec<ReferenceMatchDto> = Vec::new();
    let mut i = 0usize;
    'outer: while i < words.len() {
        let max_j = (i + MAX_REF_WORDS).min(words.len());
        for j in (i + 1..=max_j).rev() {
            let raw = &text[words[i].0..words[j - 1].1];
            let Some(parsed) = sermon_core::reference::resolve(raw) else {
                continue;
            };
            // A bare "ff." with no anchor is noise in prose scanning.
            if parsed.book_num.is_none()
                && parsed.chapter.is_none()
                && parsed.verse_start.is_none()
                && parsed.open_ended
            {
                continue;
            }
            let mut dto = ReferenceMatchDto::from(parsed);
            dto.offset = words[i].0;
            dto.length = raw.len();
            out.push(dto);
            i = j;
            continue 'outer;
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Filesystem state mapping
// ---------------------------------------------------------------------------

/// Map Track C's `FileState` onto the Rocket `FilesystemState` vocabulary.
pub fn file_state_to_ts(state: FileState) -> &'static str {
    match state {
        FileState::Clean => "clean",
        FileState::DirtyLocal => "local-dirty",
        FileState::DiskChanged => "disk-changed",
        FileState::Conflict => "both-changed",
        FileState::Missing => "missing",
    }
}

// ---------------------------------------------------------------------------
// Sermon transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineNodeDto {
    pub id: String,
    pub level: u32,
    pub text: String,
    pub children: Vec<OutlineNodeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectiveEntryDto {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SermonDocumentDto {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    pub scripture: String,
    pub series: Option<String>,
    pub status: String,
    /// Canonical Markdown source. Markdown on disk remains the sermon source
    /// of truth; the TipTap HTML conversion happens in the frontend editor
    /// via the directive transport codec (never a second grammar).
    pub body: String,
    pub outline: Vec<OutlineNodeDto>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub preached_on: Option<String>,
    pub version: u32,
    pub directives: Vec<DirectiveEntryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fs_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SermonSummaryDto {
    pub id: String,
    pub title: String,
    pub scripture: String,
    pub series: Option<String>,
    pub status: String,
    pub word_count: u32,
    pub created_at: String,
    pub updated_at: String,
    pub preached_on: Option<String>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fs_state: Option<String>,
}

/// Derive an outline (nested by heading level) from the canonical AST's
/// verbatim Markdown blocks. Thin composition over Track B's `Sermon`.
pub fn outline_from_markdown(markdown: &str) -> Vec<OutlineNodeDto> {
    let mut roots: Vec<OutlineNodeDto> = Vec::new();
    // Stack of (level, index-path) — indexes into nested children vectors.
    let mut stack: Vec<(u32, Vec<usize>)> = Vec::new();
    for (n, line) in markdown.lines().enumerate() {
        let t = line.trim_start();
        let hashes = t.chars().take_while(|&c| c == '#').count();
        if !(1..=6).contains(&hashes) {
            continue;
        }
        // ATX heading requires a space after the hashes.
        if t.chars().nth(hashes) != Some(' ') {
            continue;
        }
        let text = t[hashes..].trim().to_string();
        if text.is_empty() {
            continue;
        }
        let level = hashes as u32;
        let node = OutlineNodeDto {
            id: format!("h-{n}"),
            level,
            text,
            children: Vec::new(),
        };
        while stack.last().map(|(l, _)| *l) >= Some(level) {
            stack.pop();
        }
        match stack.last() {
            None => {
                roots.push(node);
                stack.push((level, vec![roots.len() - 1]));
            }
            Some((_, path)) => {
                let mut target = &mut roots;
                for &idx in path {
                    target = &mut target[idx].children;
                }
                target.push(node);
                let mut new_path = path.clone();
                new_path.push(target.len() - 1);
                stack.push((level, new_path));
            }
        }
    }
    roots
}

// ---------------------------------------------------------------------------
// Save / conflict transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfoDto {
    pub local_title: String,
    pub local_modified_at: String,
    pub disk_modified_at: String,
    pub disk_version: u32,
    pub disk_word_count: u32,
    pub source_path: String,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResultDto {
    pub success: bool,
    pub saved_at: String,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<ConflictInfoDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolutionDto {
    pub sermon_id: String,
    pub strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_as_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Diff / merge transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLineDto {
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunkDto {
    pub local_start: usize,
    pub local_count: usize,
    pub disk_start: usize,
    pub disk_count: usize,
    pub lines: Vec<DiffLineDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffPreparationResultDto {
    pub local_lines: Vec<String>,
    pub disk_lines: Vec<String>,
    pub hunks: Vec<DiffHunkDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePreparationResultDto {
    pub base: String,
    pub local: String,
    pub disk: String,
    /// Inline merge-conflict markers are not auto-detected; the three full
    /// versions are provided so the frontend's 3-way merge UI can operate.
    pub conflicts: Vec<serde_json::Value>,
}

/// Simple line diff (LCS). Sermon-scale inputs only; for very large inputs it
/// degrades to a single prefix/suffix-trimmed replace block rather than
/// quadratic blowup.
pub fn diff_lines(local: &[String], disk: &[String]) -> Vec<DiffLineDto> {
    let n = local.len();
    let m = disk.len();
    if n * m > 4_000_000 {
        return diff_lines_trim(local, disk);
    }
    // LCS lengths table.
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if local[i] == disk[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if local[i] == disk[j] {
            out.push(DiffLineDto { kind: "context".into(), text: local[i].clone() });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push(DiffLineDto { kind: "removed".into(), text: local[i].clone() });
            i += 1;
        } else {
            out.push(DiffLineDto { kind: "added".into(), text: disk[j].clone() });
            j += 1;
        }
    }
    while i < n {
        out.push(DiffLineDto { kind: "removed".into(), text: local[i].clone() });
        i += 1;
    }
    while j < m {
        out.push(DiffLineDto { kind: "added".into(), text: disk[j].clone() });
        j += 1;
    }
    out
}

fn diff_lines_trim(local: &[String], disk: &[String]) -> Vec<DiffLineDto> {
    let mut start = 0usize;
    while start < local.len().min(disk.len()) && local[start] == disk[start] {
        start += 1;
    }
    let mut end_l = local.len();
    let mut end_d = disk.len();
    while end_l > start && end_d > start && local[end_l - 1] == disk[end_d - 1] {
        end_l -= 1;
        end_d -= 1;
    }
    let mut out = Vec::new();
    for line in &local[..start] {
        out.push(DiffLineDto { kind: "context".into(), text: line.clone() });
    }
    for line in &local[start..end_l] {
        out.push(DiffLineDto { kind: "removed".into(), text: line.clone() });
    }
    for line in &disk[start..end_d] {
        out.push(DiffLineDto { kind: "added".into(), text: line.clone() });
    }
    for line in &local[end_l..] {
        out.push(DiffLineDto { kind: "context".into(), text: line.clone() });
    }
    out
}

/// Group a flat diff into hunks with up to 3 lines of context around each
/// change run (runs closer than 2*context+1 lines merge into one hunk).
pub fn hunks_from_diff(lines: &[DiffLineDto], context: usize) -> Vec<DiffHunkDto> {
    let changed: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind != "context")
        .map(|(i, _)| i)
        .collect();
    if changed.is_empty() {
        return Vec::new();
    }
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let (mut s, mut e) = (changed[0], changed[0]);
    for &c in &changed[1..] {
        if c <= e + 2 * context + 1 {
            e = c;
        } else {
            ranges.push((s, e));
            s = c;
            e = c;
        }
    }
    ranges.push((s, e));

    let mut hunks = Vec::new();
    for (cs, ce) in ranges {
        let lo = cs.saturating_sub(context);
        let hi = (ce + context + 1).min(lines.len());
        let slice = &lines[lo..hi];
        let mut local_start = 0usize;
        let mut disk_start = 0usize;
        for l in &lines[..lo] {
            if l.kind != "added" {
                local_start += 1;
            }
            if l.kind != "removed" {
                disk_start += 1;
            }
        }
        let local_count = slice.iter().filter(|l| l.kind != "added").count();
        let disk_count = slice.iter().filter(|l| l.kind != "removed").count();
        hunks.push(DiffHunkDto {
            local_start,
            local_count,
            disk_start,
            disk_count,
            lines: slice.to_vec(),
        });
    }
    hunks
}

// ---------------------------------------------------------------------------
// Filesystem reconciliation transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemReconciliationStatusDto {
    pub sermon_id: String,
    pub state: String,
    pub source_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_modified_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_modified_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renamed_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryResultDto {
    pub success: bool,
    pub recovered_path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconnectRequestDto {
    pub sermon_id: String,
    pub new_path: String,
}

// ---------------------------------------------------------------------------
// Search / study / index transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFiltersDto {
    #[serde(default)]
    pub status: Vec<String>,
    #[serde(default)]
    pub series: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub scripture_book: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultDto {
    pub id: String,
    pub title: String,
    pub scripture: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PassageResultDto {
    pub reference: String,
    pub text: String,
    pub translation: String,
    pub verses: Vec<VerseEntryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub osis_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerseEntryDto {
    pub verse: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrongsEntryDto {
    pub id: String,
    pub lemma: String,
    pub transliteration: String,
    pub definition: String,
    pub gloss: String,
    pub part_of_speech: String,
    pub occurrences: u32,
    #[serde(default)]
    pub usage_examples: Vec<serde_json::Value>,
    #[serde(default)]
    pub related_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossReferenceDto {
    pub reference: String,
    pub snippet: String,
    pub relevance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreachedResultDto {
    pub sermon_id: String,
    pub sermon_title: String,
    pub preached_on: String,
    pub series: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexOperationResultDto {
    pub success: bool,
    pub message: String,
    pub documents_indexed: u32,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatusDto {
    pub indexed_file_count: u32,
    pub index_version: String,
    pub last_reconciliation_time: Option<String>,
    pub last_full_scan_time: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveStatsDto {
    pub total_sermons: i64,
    pub total_series: i64,
    pub total_words: i64,
    pub last_preached_on: Option<String>,
    pub oldest_sermon: Option<String>,
    pub newest_sermon: Option<String>,
    #[serde(default)]
    pub sermons_by_status: std::collections::HashMap<String, i64>,
    #[serde(default)]
    pub sermons_by_month: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationFatigueDto {
    pub illustration: String,
    pub use_count: i64,
    pub last_used_in: String,
    pub last_used_on: String,
    pub severity: String,
}

// ---------------------------------------------------------------------------
// Export / lint transport. Business rules remain in sermon_core::export and
// sermon_core::linter; these types only map their results to the frontend.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRangeDto {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LintFindingDto {
    pub id: String,
    pub severity: String,
    pub rule_id: String,
    /// Legacy frontend alias for ruleId.
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movement_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_range: Option<SourceRangeDto>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptionsDto {
    #[serde(default)]
    pub include_notes: Option<bool>,
    #[serde(default)]
    pub output_filename: Option<String>,
    #[serde(default)]
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequestDto {
    #[serde(default)]
    pub sermon_id: String,
    pub format: String,
    #[serde(default)]
    pub manuscript_mode: Option<String>,
    #[serde(default)]
    pub options: ExportOptionsDto,
    #[serde(default)]
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExportSnapshotRequestDto {
    #[serde(default)]
    pub sermon_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSnapshotDto {
    pub snapshot_id: String,
    pub sermon_id: String,
    pub sermon_title: String,
    pub created_at: String,
    pub revision_hash: String,
    pub word_count: u32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResultDto {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub message: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exported_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size_bytes: Option<u64>,
}

// ---------------------------------------------------------------------------
// Settings transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    pub library_path: String,
    pub librarian_enabled: bool,
    pub default_translation: String,
    pub autosave_interval_seconds: u32,
    pub editor_font_size: u32,
    pub editor_font: String,
    pub spellcheck: bool,
    pub focus_mode: bool,
    #[serde(default)]
    pub export_defaults: serde_json::Value,
    #[serde(default)]
    pub keyboard_shortcuts: std::collections::HashMap<String, String>,
    pub developer_mode: bool,
}

// ---------------------------------------------------------------------------
// Codec test transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDirectiveDto {
    pub kind: String,
    pub name: String,
    pub attributes: std::collections::HashMap<String, String>,
    pub body: String,
    pub raw_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodecRoundTripResultDto {
    pub pass: bool,
    pub input: String,
    pub parsed: Vec<ParsedDirectiveDto>,
    pub serialized: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_transport_preserves_three_states() {
        // "John 3:16-5" is structurally a reference with an impossible range,
        // which the core resolver reports as Invalid; unknown-book prose like
        // "Hezekiah 99:199" is not a reference at all and is correctly absent.
        let src = "see John 3:16, also v.6, and John 3:16-5 odd";
        let hits = scan_references(src);
        let states: Vec<ResolutionDto> = hits.iter().map(|h| h.resolution).collect();
        assert!(states.contains(&ResolutionDto::Definite), "{hits:?}");
        assert!(states.contains(&ResolutionDto::Ambiguous), "{hits:?}");
        assert!(states.contains(&ResolutionDto::Invalid), "{hits:?}");
        // Definite hit carries canonical form and its exact raw source slice
        // (trailing punctuation preserved as evidence).
        let definite = hits.iter().find(|h| h.resolution == ResolutionDto::Definite).unwrap();
        assert_eq!(definite.osis_id.as_deref(), Some("John.3.16"));
        assert_eq!(definite.raw, "John 3:16,");
        assert_eq!(definite.book, "John");
        // Invalid is never clamped: evidence preserved, no canonical form.
        let invalid = hits.iter().find(|h| h.resolution == ResolutionDto::Invalid).unwrap();
        assert_eq!(invalid.osis_id, None);
        assert!(invalid.reason.is_some());
        assert!(invalid.raw.contains("John 3:16-5"));
        // Offsets/lengths point back into the source text.
        for h in &hits {
            assert_eq!(&src[h.offset..h.offset + h.length], h.raw.as_str());
        }
    }

    #[test]
    fn reference_scanner_handles_ranges_and_roman_numerals() {
        let hits = scan_references("Rom. viii. 28-30 and II Cor 3:18ff");
        let first = hits.first().unwrap();
        assert_eq!(first.resolution, ResolutionDto::Definite);
        assert_eq!(first.osis_id.as_deref(), Some("Rom.8.28-Rom.8.30"));
        assert!(hits.iter().any(|h| h.raw.contains("ff")));
    }

    #[test]
    fn file_state_maps_to_rocket_vocabulary() {
        assert_eq!(file_state_to_ts(FileState::Clean), "clean");
        assert_eq!(file_state_to_ts(FileState::DirtyLocal), "local-dirty");
        assert_eq!(file_state_to_ts(FileState::DiskChanged), "disk-changed");
        assert_eq!(file_state_to_ts(FileState::Conflict), "both-changed");
        assert_eq!(file_state_to_ts(FileState::Missing), "missing");
    }

    #[test]
    fn settings_transport_roundtrips() {
        let cfg = crate::config::AppConfig {
            vault_path: "/vault".into(),
            canon_path: "/canon.db".into(),
            pastor_path: "/pastor.db".into(),
            librarian_enabled: true,
            font_size: 21,
            high_contrast: false,
        };
        let dto = crate::core_api::settings_from_config(&cfg);
        assert_eq!(dto.library_path, "/vault");
        assert_eq!(dto.editor_font_size, 21);
        let back = crate::core_api::settings_to_config(&dto, &cfg);
        assert_eq!(back.vault_path, "/vault");
        assert_eq!(back.librarian_enabled, true);
        assert_eq!(back.font_size, 21);
        assert_eq!(back.canon_path, "/canon.db");
        assert_eq!(back.pastor_path, "/pastor.db");
    }

    #[test]
    fn outline_nests_by_heading_level() {
        let md = "# One\n\n## One-A\n\ntext\n\n## One-B\n\n# Two\n\n### Two-A-deep\n";
        let outline = outline_from_markdown(md);
        assert_eq!(outline.len(), 2);
        assert_eq!(outline[0].text, "One");
        assert_eq!(outline[0].children.len(), 2);
        assert_eq!(outline[0].children[0].text, "One-A");
        assert_eq!(outline[1].text, "Two");
        assert_eq!(outline[1].children[0].text, "Two-A-deep");
    }

    #[test]
    fn diff_and_hunks_classify_changes() {
        let local: Vec<String> = "a\nb\nc\nd\ne".lines().map(|s| s.to_string()).collect();
        let disk: Vec<String> = "a\nB\nc\nd\nE\nf".lines().map(|s| s.to_string()).collect();
        let diff = diff_lines(&local, &disk);
        let removed = diff.iter().filter(|l| l.kind == "removed").count();
        let added = diff.iter().filter(|l| l.kind == "added").count();
        assert_eq!(removed, 2);
        assert_eq!(added, 3);
        let hunks = hunks_from_diff(&diff, 3);
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0].lines.iter().any(|l| l.kind == "context"));
        // Identical inputs: no hunks.
        let same = diff_lines(&local, &local);
        assert!(hunks_from_diff(&same, 3).is_empty());
    }

    #[test]
    fn identical_inputs_diff_clean() {
        let l: Vec<String> = "x\ny\nz".lines().map(|s| s.to_string()).collect();
        let d = diff_lines(&l, &l);
        assert!(d.iter().all(|x| x.kind == "context"));
    }
}
