#!/usr/bin/env python3
"""Post-build verification of a production canon.db.

Reads data/canon.db READ-ONLY and reports the production data-quality gates plus
the sample lookups required by Track M. Mirrors `crates/core/src/canon/validate.rs`.

Usage:  python3 tools/verify_canon_data.py [canon.db]
"""
import json
import os
import sqlite3
import sys


def q(conn, sql, args=()):
    return conn.execute(sql, args).fetchone()[0]


def main():
    db = sys.argv[1] if len(sys.argv) > 1 else "data/canon.db"
    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)

    print("=" * 70)
    print("PRODUCTION canon.db — data-quality gates")
    print("=" * 70)
    errors = []
    warnings = []

    book_count = q(conn, "SELECT COUNT(*) FROM bible_books")
    verse_count = q(conn, "SELECT COUNT(*) FROM bible_verses")
    lexicon_count = q(conn, "SELECT COUNT(*) FROM strongs_lexicon")
    vw_count = q(conn, "SELECT COUNT(*) FROM verse_words")
    xref_count = q(conn, "SELECT COUNT(*) FROM cross_references")
    topic_count = q(conn, "SELECT COUNT(*) FROM topics")
    tv_count = q(conn, "SELECT COUNT(*) FROM topic_verses")
    source_count = q(conn, "SELECT COUNT(*) FROM sources")

    print(f"books   = {book_count}")
    print(f"verses  = {verse_count}")
    print(f"lexicon = {lexicon_count}")
    print(f"words   = {vw_count}")
    print(f"xrefs   = {xref_count}")
    print(f"topics  = {topic_count}")
    print(f"topic_verses = {tv_count}")
    print(f"sources = {source_count}")

    if book_count != 66:
        errors.append(f"expected 66 books, found {book_count}")

    distinct = q(conn, "SELECT COUNT(*) FROM (SELECT DISTINCT book_num, chapter, verse FROM bible_verses)")
    if distinct != verse_count:
        errors.append(f"duplicate verse identity: {verse_count} rows vs {distinct} distinct")

    malformed = q(conn, "SELECT COUNT(*) FROM strongs_lexicon WHERE strong_id NOT GLOB '[GH][0-9]*'")
    if malformed:
        errors.append(f"{malformed} malformed Strong's ids")

    dangling_vw = q(conn, "SELECT COUNT(*) FROM verse_words vw LEFT JOIN bible_verses bv ON vw.verse_id=bv.id WHERE bv.id IS NULL")
    if dangling_vw:
        errors.append(f"{dangling_vw} verse_words rows reference a missing verse")

    dangling_xr = q(conn, "SELECT COUNT(*) FROM cross_references cr LEFT JOIN bible_verses f ON cr.from_verse_id=f.id LEFT JOIN bible_verses t ON cr.to_verse_id=t.id WHERE f.id IS NULL OR t.id IS NULL")
    if dangling_xr:
        errors.append(f"{dangling_xr} cross_references rows have a missing endpoint")

    dangling_tv = q(conn, "SELECT COUNT(*) FROM topic_verses tv LEFT JOIN bible_verses bv ON tv.verse_id=bv.id WHERE bv.id IS NULL")
    if dangling_tv:
        errors.append(f"{dangling_tv} topic_verses rows reference a missing verse")

    for table, col in [("topics", "source_id"), ("topic_verses", "source_id"), ("chain_edges", "source_id"), ("cross_references", "source_id")]:
        orphan = q(conn, f"SELECT COUNT(*) FROM {table} t LEFT JOIN sources s ON t.{col}=s.id WHERE s.id IS NULL")
        if orphan:
            errors.append(f"{orphan} orphan source_id in {table}.{col}")

    if lexicon_count > 0:
        dangling_strong = q(conn, "SELECT COUNT(*) FROM verse_words vw WHERE vw.strong_id IS NOT NULL AND vw.strong_id NOT IN (SELECT strong_id FROM strongs_lexicon)")
        if dangling_strong:
            warnings.append(f"{dangling_strong} verse_words strong_ids lack a lexicon entry")

    enriched_vw = q(conn, "SELECT COUNT(*) FROM verse_words WHERE source_id IS NOT NULL")
    if vw_count > 0 and enriched_vw == 0:
        warnings.append("verse_words carries no source provenance")
    enriched_xr = q(conn, "SELECT COUNT(*) FROM cross_references WHERE source_id IS NOT NULL")
    if xref_count > 0 and enriched_xr == 0:
        warnings.append("cross_references carries no source provenance")

    print("\n-- Gates --")
    for e in errors:
        print("  ERROR:", e)
    for w in warnings:
        print("  WARN :", w)
    print("  RESULT:", "PASS" if not errors else "FAIL", f"({len(errors)} errors, {len(warnings)} warnings)")

    print()
    print("=" * 70)
    print("PRODUCTION DATA COVERAGE")
    print("=" * 70)
    greek = q(conn, "SELECT COUNT(*) FROM strongs_lexicon WHERE testament='NT'")
    hebrew = q(conn, "SELECT COUNT(*) FROM strongs_lexicon WHERE testament='OT'")
    print(f"lexicon total  = {lexicon_count}")
    print(f"lexicon Greek  = {greek}")
    print(f"lexicon Hebrew = {hebrew}")

    morph = q(conn, "SELECT COUNT(*) FROM verse_words WHERE morphology_code IS NOT NULL")
    lemma = q(conn, "SELECT COUNT(*) FROM verse_words WHERE lemma IS NOT NULL")
    gloss = q(conn, "SELECT COUNT(*) FROM verse_words WHERE gloss IS NOT NULL")
    ext = q(conn, "SELECT COUNT(*) FROM verse_words WHERE strongs_extended IS NOT NULL")
    print(f"verse_words total               = {vw_count}")
    print(f"  morphology coverage            = {morph} ({100.0*morph/vw_count:.1f}%)")
    print(f"  lemma coverage                 = {lemma} ({100.0*lemma/vw_count:.1f}%)")
    print(f"  gloss coverage                 = {gloss} ({100.0*gloss/vw_count:.1f}%)")
    print(f"  extended Strong's coverage     = {ext} ({100.0*ext/vw_count:.1f}%)")

    print()
    print("=" * 70)
    print("SAMPLE LOOKUPS")
    print("=" * 70)

    for sid in ("G25", "G26", "H430", "H7225"):
        row = conn.execute(
            "SELECT strong_id, testament, lemma, transliteration, pronunciation, part_of_speech, definition, gloss, source_id, source_version FROM strongs_lexicon WHERE strong_id=?",
            (sid,),
        ).fetchone()
        if row:
            print(f"\nStrong's {row[0]} ({row[1]}) lemma={row[2]} translit={row[3]} pos={row[5]}")
            print(f"    gloss: {row[7]}")
            print(f"    def  : {row[6][:110]}")
            print(f"    src  : {row[8]} v{row[9]}")
        else:
            print(f"\nStrong's {sid}: NOT FOUND")

    def show_words(book, ch, vs, label):
        print(f"\n{label} (book {book}:{ch}:{vs}) interlinear:")
        for w in conn.execute(
            "SELECT word_order, surface_word, strong_id, morphology_code, strongs_extended, lemma, gloss FROM verse_words vw JOIN bible_verses v ON v.id=vw.verse_id WHERE v.book_num=? AND v.chapter=? AND v.verse=? ORDER BY word_order LIMIT 6",
            (book, ch, vs),
        ).fetchall():
            print(f"  [{w[0]:>2}] {w[1][:18]:<18} strong={w[2]} morph={w[3]} ext={w[4]} lemma={w[5]} gloss={w[6]}")

    show_words(43, 1, 1, "NT Greek morphology (John 1:1)")
    show_words(1, 1, 1, "OT Hebrew morphology (Gen 1:1)")

    print("\n-- Topics --")
    for source_id, tid in [("naves-topical", "nave:love"), ("torrey-topical", "torrey:faith")]:
        t = conn.execute("SELECT id, name, source_version FROM topics WHERE id=?", (tid,)).fetchone()
        if t:
            tv = q(conn, "SELECT COUNT(*) FROM topic_verses WHERE topic_id=?", (tid,))
            print(f"  {source_id}: topic '{t[1]}' (id={t[0]}, v{t[2]}) -> {tv} verses")
            sample = conn.execute(
                "SELECT v.book_num, v.chapter, v.verse FROM topic_verses tv JOIN bible_verses v ON v.id=tv.verse_id WHERE tv.topic_id=? ORDER BY v.book_num, v.chapter, v.verse LIMIT 3",
                (tid,),
            ).fetchall()
            print(f"      sample refs: {[(b,c,v) for (b,c,v) in sample]}")
        else:
            print(f"  {source_id}: topic {tid} NOT FOUND")

    # A concrete Nave's + Torrey's sample name (Love/Faith) from the DB by name.
    print("\n  Nave's topic names (sample):", [r[0] for r in conn.execute("SELECT name FROM topics WHERE source_id='naves-topical' ORDER BY id LIMIT 3").fetchall()])
    print("  Torrey's topic names (sample):", [r[0] for r in conn.execute("SELECT name FROM topics WHERE source_id='torrey-topical' ORDER BY id LIMIT 3").fetchall()])

    print()
    print("=" * 70)
    print("MANIFEST + SOURCES")
    print("=" * 70)
    m = conn.execute("SELECT value FROM canon_meta WHERE key='manifest'").fetchone()
    if m:
        manifest = json.loads(m[0])
        print(f"canon_version = {manifest['canon_version']}")
        print(f"build_ts      = {manifest['build_timestamp']}")
        print(f"source_versions ({len(manifest['source_versions'])}):")
        for k in sorted(manifest["source_versions"]):
            print(f"    {k}: {manifest['source_versions'][k]}")
        print(f"source_checksums ({len(manifest['source_checksums'])}):")
        for k in sorted(manifest["source_checksums"]):
            print(f"    {k}: {manifest['source_checksums'][k][:16]}…")

    conn.close()

    size = os.path.getsize(db)
    print()
    print("=" * 70)
    print(f"canon.db size = {size:,} bytes ({size/1024/1024:.1f} MB)")
    import hashlib
    h = hashlib.sha256()
    with open(db, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    print(f"canon.db sha256 = {h.hexdigest()}")
    print("=" * 70)


if __name__ == "__main__":
    main()
