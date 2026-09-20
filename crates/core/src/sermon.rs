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
