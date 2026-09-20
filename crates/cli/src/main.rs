//! Sermon Studio headless engine.
//!
//! A thin CLI over `sermon_core` for building the canon vault, indexing a
//! sermon vault, and querying the retrieval engine. Useful for scripting,
//! cron-based re-indexing, and verifying the engine without the GUI.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sermon_core::{canon, indexer, reference, retrieval, sermon};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sermon", version, about = "Sermon Studio headless engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build canon.db from a normalized (ETL) data directory.
    BuildCanon {
        #[arg(long, default_value = "data/clean")]
        clean: PathBuf,
        #[arg(long, default_value = "canon.db")]
        out: PathBuf,
    },
    /// Full rebuild of pastor.db from a sermon vault.
    Index {
        #[arg(long)]
        vault: PathBuf,
        #[arg(long, default_value = "pastor.db")]
        db: PathBuf,
    },
    /// Incremental sync of pastor.db.
    Sync {
        #[arg(long)]
        vault: PathBuf,
        #[arg(long, default_value = "pastor.db")]
        db: PathBuf,
    },
    /// Full-text search the sermon archive.
    Search {
        query: String,
        #[arg(long, default_value = "pastor.db")]
        db: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    /// Look up a verse (KJV) plus its Strong's words and cross-references.
    Verse {
        /// Reference, e.g. "Rom 8:28" or "John 3:16".
        reference: String,
        #[arg(long, default_value = "canon.db")]
        canon: PathBuf,
    },
    /// Look up a Strong's lexicon entry.
    Strong {
        strong_id: String,
        #[arg(long, default_value = "canon.db")]
        canon: PathBuf,
    },
    /// Sermons that touch a verse.
    Preached {
        reference: String,
        #[arg(long, default_value = "pastor.db")]
        db: PathBuf,
    },
    /// Illustration fatigue report.
    Fatigue {
        #[arg(long, default_value = "pastor.db")]
        db: PathBuf,
        #[arg(long, default_value_t = 2)]
        min: i64,
    },
    /// Archive statistics.
    Stats {
        #[arg(long, default_value = "pastor.db")]
        db: PathBuf,
    },
    /// Create a new sermon template file.
    New {
        title: String,
        #[arg(long, default_value = "")]
        passage: String,
        #[arg(long)]
        vault: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::BuildCanon { clean, out } => {
            let stats = canon::build_canon_db(&clean, &out).context("building canon.db")?;
            println!(
                "canon.db built: {} books, {} verses, {} lexicon entries, {} verse-words, {} cross-refs",
                stats.books, stats.verses, stats.lexicon, stats.verse_words, stats.cross_references
            );
        }
        Command::Index { vault, db } => {
            let stats = indexer::rebuild(&vault, &db).context("rebuilding pastor.db")?;
            println!(
                "indexed {} sermons ({} scanned, {} links, {} illustrations) in {} ms",
                stats.indexed, stats.scanned, stats.links, stats.illustrations, stats.elapsed_ms
            );
        }
        Command::Sync { vault, db } => {
            let stats = indexer::sync(&vault, &db).context("syncing pastor.db")?;
            println!(
                "synced: {} indexed, {} unchanged, {} removed in {} ms",
                stats.indexed, stats.skipped_unchanged, stats.removed, stats.elapsed_ms
            );
        }
        Command::Search { query, db, limit } => {
            let conn = indexer::open_pastor_db(&db)?;
            let hits = retrieval::search_sermons(&conn, &query, limit)?;
            if hits.is_empty() {
                println!("(no matches)");
            }
            for h in hits {
                println!("• {}  [{}]", h.title, h.primary_passage);
                if let Some(d) = &h.date_preached {
                    println!("    preached: {}", d);
                }
                println!("    {}", h.snippet.replace('\n', " "));
            }
        }
        Command::Verse { reference, canon } => {
            let conn = sermon_core::open_canon_readonly(&canon)?;
            let p = reference::parse_passage(&reference)?;
            let verses = retrieval::get_passage(&conn, p.start, p.end)?;
            for v in &verses {
                println!("{} {}:{}  {}", v.book_name, v.chapter, v.verse, v.text_kjv);
            }
            if let Some(first) = verses.first() {
                println!("\n-- Strong's words --");
                for w in retrieval::get_verse_words(&conn, first.book_num, first.chapter, first.verse)? {
                    match &w.strong_id {
                        Some(s) => println!("  {:>3}. {} [{}]", w.word_order, w.surface_word, s),
                        None => println!("  {:>3}. {}", w.word_order, w.surface_word),
                    }
                }
                println!("\n-- Cross references --");
                for (x, rank) in retrieval::cross_references(&conn, first.book_num, first.chapter, first.verse, 10)? {
                    println!("  ({}) {} {}:{}", rank, x.book_name, x.chapter, x.verse);
                }
            }
        }
        Command::Strong { strong_id, canon } => {
            let conn = sermon_core::open_canon_readonly(&canon)?;
            match retrieval::get_lexicon(&conn, &strong_id)? {
                Some(e) => {
                    println!("{}  {}  ({})", e.strong_id, e.lemma, e.transliteration);
                    if let Some(p) = &e.pronunciation {
                        println!("pron: {}", p);
                    }
                    println!("gloss: {}", e.gloss);
                    println!("def: {}", e.definition);
                    if let Some(d) = &e.derivation {
                        println!("derivation: {}", d);
                    }
                }
                None => println!("(not found)"),
            }
        }
        Command::Preached { reference, db } => {
            let conn = indexer::open_pastor_db(&db)?;
            let p = reference::parse_passage(&reference)?;
            let hits = retrieval::sermons_for_verse(&conn, p.start.book_num, p.start.chapter, p.start.verse)?;
            if hits.is_empty() {
                println!("(no sermons touch this verse)");
            }
            for h in hits {
                println!("• {}  [{}]  {}", h.title, h.primary_passage, h.date_preached.unwrap_or_default());
            }
        }
        Command::Fatigue { db, min } => {
            let conn = indexer::open_pastor_db(&db)?;
            for f in retrieval::illustration_fatigue(&conn, min)? {
                println!("{}×  {}  ({} sermons, last {})", f.total_uses, f.label, f.sermon_count, f.last_used.unwrap_or_default());
            }
        }
        Command::Stats { db } => {
            let conn = indexer::open_pastor_db(&db)?;
            let s = retrieval::archive_stats(&conn)?;
            println!("sermons: {}", s.sermon_count);
            println!("series: {}", s.series_count);
            println!("verse links: {}", s.verse_links);
            println!("illustrations: {}", s.illustration_count);
            println!("last rebuild: {}", s.last_rebuild.unwrap_or_default());
        }
        Command::New { title, passage, vault } => {
            std::fs::create_dir_all(&vault)?;
            let slug = sermon::slugify(&title);
            let path = vault.join(format!("{}.md", slug));
            let content = sermon::new_sermon_template(&title, &passage);
            std::fs::write(&path, content)?;
            println!("created {}", path.display());
        }
    }
    Ok(())
}
