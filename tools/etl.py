#!/usr/bin/env python3
"""Sermon Studio — deterministic canon.db ETL.

Normalizes local/raw public-domain source files into the deterministic TSVs the
Rust `canon.db` builder ingests. Runtime never downloads: acquisition is a
separate, offline, human-run step; this script runs only at build time.

Outputs (data/clean/):
  verses.tsv        book_num, chapter, verse, text
  lexicon.tsv       strong_id, testament, lemma, transliteration, pronunciation,
                    part_of_speech, definition, gloss, derivation, usage_note,
                    source_id, source_version
  verse_words.tsv   book_num, chapter, verse, word_order, surface, strong_id,
                    morphology, strongs_extended, lemma, gloss, source_id
  xrefs.tsv         from_b, from_c, from_v, to_b, to_c, to_v, rank, weight, source_id
  topics.tsv        topic_id, name, source_id, source_version
  topic_verses.tsv  topic_id, book_num, chapter, verse, weight, source_id
  etl-manifest.json machine-readable ETL version, raw/output checksums, row counts,
                    warnings/errors

Determinism contract: identical raw inputs + this ETL version => byte-identical
normalized output (stable ordering, stable ids, NFC Unicode, single-space
whitespace, LF newlines). Invalid verse references are REJECTED, never clamped.

Run:  python3 tools/etl.py --raw data/raw --out data/clean
Self-test: python3 tools/etl.py --self-test
"""
import argparse
import csv
import hashlib
import json
import os
import re
import sys
import unicodedata

ETL_VERSION = "2025.01-track-m"

# ── Canonical 66 books + the ONE abbreviation-mapping table ──────────────────
BOOKS = [
    (1, "Gen", "Genesis", "OT"), (2, "Exod", "Exodus", "OT"), (3, "Lev", "Leviticus", "OT"),
    (4, "Num", "Numbers", "OT"), (5, "Deut", "Deuteronomy", "OT"), (6, "Josh", "Joshua", "OT"),
    (7, "Judg", "Judges", "OT"), (8, "Ruth", "Ruth", "OT"), (9, "1Sam", "1 Samuel", "OT"),
    (10, "2Sam", "2 Samuel", "OT"), (11, "1Kgs", "1 Kings", "OT"), (12, "2Kgs", "2 Kings", "OT"),
    (13, "1Chr", "1 Chronicles", "OT"), (14, "2Chr", "2 Chronicles", "OT"), (15, "Ezra", "Ezra", "OT"),
    (16, "Neh", "Nehemiah", "OT"), (17, "Esth", "Esther", "OT"), (18, "Job", "Job", "OT"),
    (19, "Ps", "Psalms", "OT"), (20, "Prov", "Proverbs", "OT"), (21, "Eccl", "Ecclesiastes", "OT"),
    (22, "Song", "Song of Solomon", "OT"), (23, "Isa", "Isaiah", "OT"), (24, "Jer", "Jeremiah", "OT"),
    (25, "Lam", "Lamentations", "OT"), (26, "Ezek", "Ezekiel", "OT"), (27, "Dan", "Daniel", "OT"),
    (28, "Hos", "Hosea", "OT"), (29, "Joel", "Joel", "OT"), (30, "Amos", "Amos", "OT"),
    (31, "Obad", "Obadiah", "OT"), (32, "Jonah", "Jonah", "OT"), (33, "Mic", "Micah", "OT"),
    (34, "Nah", "Nahum", "OT"), (35, "Hab", "Habakkuk", "OT"), (36, "Zeph", "Zephaniah", "OT"),
    (37, "Hag", "Haggai", "OT"), (38, "Zech", "Zechariah", "OT"), (39, "Mal", "Malachi", "OT"),
    (40, "Matt", "Matthew", "NT"), (41, "Mark", "Mark", "NT"), (42, "Luke", "Luke", "NT"),
    (43, "John", "John", "NT"), (44, "Acts", "Acts", "NT"), (45, "Rom", "Romans", "NT"),
    (46, "1Cor", "1 Corinthians", "NT"), (47, "2Cor", "2 Corinthians", "NT"), (48, "Gal", "Galatians", "NT"),
    (49, "Eph", "Ephesians", "NT"), (50, "Phil", "Philippians", "NT"), (51, "Col", "Colossians", "NT"),
    (52, "1Thess", "1 Thessalonians", "NT"), (53, "2Thess", "2 Thessalonians", "NT"),
    (54, "1Tim", "1 Timothy", "NT"), (55, "2Tim", "2 Timothy", "NT"), (56, "Titus", "Titus", "NT"),
    (57, "Phlm", "Philemon", "NT"), (58, "Heb", "Hebrews", "NT"), (59, "Jas", "James", "NT"),
    (60, "1Pet", "1 Peter", "NT"), (61, "2Pet", "2 Peter", "NT"), (62, "1John", "1 John", "NT"),
    (63, "2John", "2 John", "NT"), (64, "3John", "3 John", "NT"), (65, "Jude", "Jude", "NT"),
    (66, "Rev", "Revelation", "NT"),
]

# Aliases seen in the wild (lowercased, alnum-only key -> book_num).
ALIASES = {
    "i chronicles": 13, "ii chronicles": 14, "i corinthians": 46, "ii corinthians": 47,
    "i john": 62, "ii john": 63, "iii john": 64, "i kings": 11, "ii kings": 12,
    "i peter": 60, "ii peter": 61, "i samuel": 9, "ii samuel": 10,
    "i thessalonians": 52, "ii thessalonians": 53, "i timothy": 54, "ii timothy": 55,
    "revelation of john": 66, "revelation": 66, "psalm": 19, "psalms": 19,
    "song of solomon": 22, "song of songs": 22, "canticles": 22,
    "1co": 46, "2co": 47, "1jo": 62, "2jo": 63, "3jo": 64, "1ki": 11, "2ki": 12,
    "1pe": 60, "2pe": 61, "1sa": 9, "2sa": 10, "1th": 52, "2th": 53, "1ti": 54, "2ti": 55,
    "1ch": 13, "2ch": 14, "sng": 22, "psa": 19, "jhn": 43, "mar": 41, "luk": 42,
    "act": 44, "phl": 50, "phm": 57, "jde": 65, "oba": 31, "rth": 8, "zep": 36,
    "zec": 38, "eze": 26, "ecc": 21, "est": 17, "ezr": 15, "hab": 35, "hag": 37,
    "hos": 28, "isa": 23, "jas": 59, "jer": 24, "joe": 29, "jon": 32, "jos": 6,
    "jdg": 7, "lam": 25, "lev": 3, "mal": 39, "mat": 40, "mic": 33, "nah": 34,
    "neh": 16, "num": 4, "pro": 20, "rev": 66, "tit": 56, "amo": 30, "col": 51,
    "dan": 27, "deu": 5, "exo": 2, "gal": 48, "gen": 1, "heb": 58, "job": 18,
    "jud": 7, "rut": 8, "ex": 2, "dt": 5, "ps": 19, "pr": 20, "ec": 21, "so": 22,
    "is": 23, "jr": 24, "ez": 26, "dn": 27, "ho": 28, "jl": 29, "am": 30, "ob": 31,
    "jn": 32, "mi": 33, "na": 34, "hb": 35, "zp": 36, "hg": 37, "zc": 38, "ml": 39,
    "mt": 40, "mk": 41, "lk": 42, "jo": 43, "ac": 44, "ro": 45, "ga": 48, "ep": 49,
    "ph": 50, "cl": 51, "1ts": 52, "2ts": 53, "1tm": 54, "2tm": 55, "tt": 56,
    "1p": 60, "2p": 61, "1j": 62, "2j": 63, "3j": 64, "re": 66,
}

NAME_TO_NUM = {}
for num, osis, name, _t in BOOKS:
    for key in (osis, name):
        NAME_TO_NUM[re.sub(r"[^a-z0-9]", "", key.lower())] = num
for k, v in ALIASES.items():
    NAME_TO_NUM[re.sub(r"[^a-z0-9]", "", k.lower())] = v


def norm_book(name):
    return NAME_TO_NUM.get(re.sub(r"[^a-z0-9]", "", name.lower()))


def norm_text(s):
    return " ".join(unicodedata.normalize("NFC", s or "").split())


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def parse_csv_line(line):
    fields, cur, in_q, i = [], [], False, 0
    while i < len(line):
        c = line[i]
        if c == '"':
            if in_q and i + 1 < len(line) and line[i + 1] == '"':
                cur.append('"'); i += 2; continue
            in_q = not in_q
        elif c == ',' and not in_q:
            fields.append("".join(cur)); cur = []
        elif c == '\r':
            pass
        else:
            cur.append(c)
        i += 1
    fields.append("".join(cur))
    return fields


STRONG_TAG = re.compile(r"\[([HG]\d+)\]")
VERSE_KEY = re.compile(r'"([^"]+\|[^"]+\|[^"]+)"\s*:\s*\{\s*"en"\s*:\s*"((?:[^"\\]|\\.)*)"')
REF_RE = re.compile(r"^([1-3]?[A-Za-z]+)\.(\d+)\.(\d+)$")


def parse_ref(s):
    m = REF_RE.match(s.strip())
    if not m:
        return None
    b = norm_book(m.group(1))
    if b is None:
        return None
    return (b, int(m.group(2)), int(m.group(3)))


def parse_ref_range(s):
    """Parse a dotted reference, or a same-book/chapter range `A.B.c-A.B.d`,
    into the list of (book, chapter, verse) it covers."""
    s = s.strip()
    if '-' in s:
        left, right = s.split('-', 1)
        a = parse_ref(left)
        b = parse_ref(right)
        if not a or not b:
            return None
        if a[0] != b[0] or a[1] != b[1]:
            return None
        lo, hi = (a[2], b[2]) if a[2] <= b[2] else (b[2], a[2])
        if hi - lo > 200:
            return None
        return [(a[0], a[1], v) for v in range(lo, hi + 1)]
    r = parse_ref(s)
    return [r] if r else None


# ── Normalizers (each returns a sorted list of rows) ─────────────────────────

def etl_verses(raw, warn):
    # KJV text. Raw CSV `t_kjv.csv`: id,b,c,v,t (numeric book number, quoted text).
    src = os.path.join(raw, "t_kjv.csv")
    if not os.path.exists(src):
        return []
    rows = []
    with open(src, encoding="utf-8") as f:
        next(f, None)
        for line in f:
            if not line.strip():
                continue
            fld = parse_csv_line(line)
            if len(fld) < 5:
                continue
            try:
                b, c, v = int(fld[1]), int(fld[2]), int(fld[3])
            except ValueError:
                warn(f"verses: bad reference {fld[1]}:{fld[2]}:{fld[3]}")
                continue
            if not (1 <= b <= 66 and c >= 1 and v >= 1):
                warn(f"verses: rejected {b}:{c}:{v}")
                continue
            rows.append((b, c, v, norm_text(fld[4])))
    rows.sort(key=lambda r: (r[0], r[1], r[2]))
    return rows


def etl_lexicon(raw, warn):
    rows = []
    for fn, testament, source_id, version in (
        ("strongs-hebrew.js", "OT", "strongs-pd", "1890"),
        ("strongs-greek.js", "NT", "strongs-pd", "1890"),
        ("stepbible-tbesh.tsv", "OT", "stepbible-tbesh", "tbesh"),
        ("stepbible-tbesg.tsv", "NT", "stepbible-tbesg", "tbesg"),
    ):
        path = os.path.join(raw, fn)
        if not os.path.exists(path):
            continue
        if fn.endswith(".js"):
            text = open(path, encoding="utf-8").read()
            data = json.loads(text[text.find("{"):text.rfind("}") + 1])
            for sid, v in data.items():
                rows.append([
                    sid, testament, v.get("lemma", ""), v.get("translit") or v.get("xlit") or "",
                    v.get("pron", "") or None, v.get("part_of_speech", "") or None,
                    v.get("strongs_def", ""), v.get("kjv_def", ""), v.get("derivation", "") or None,
                    None, source_id, version,
                ])
        else:
            for line in open(path, encoding="utf-8"):
                f = [c.strip() for c in line.split("\t")]
                if len(f) < 8:
                    continue
                strong_id = f[0]
                if not (strong_id[:1] in "GH" and strong_id[1:].isdigit()):
                    warn(f"lexicon: malformed strong_id {strong_id!r}")
                    continue
                rows.append([
                    strong_id, testament, f[1], f[2], f[3] or None, f[4] or None,
                    f[5], f[6], f[7] or None, None, source_id, version,
                ])
    rows.sort(key=lambda r: r[0])
    return rows


def etl_verse_words(raw, warn):
    src_dir = os.path.join(raw, "kjv_strongs")
    rows = []
    if os.path.isdir(src_dir):
        for fn in sorted(os.listdir(src_dir)):
            if not fn.endswith(".json") or fn in ("books.json", "lexicon.json", "chapter_count.json"):
                continue
            text = open(os.path.join(src_dir, fn), encoding="utf-8").read()
            for key, en in VERSE_KEY.findall(text):
                parts = key.split("|")
                if len(parts) != 3:
                    continue
                b = norm_book(parts[0])
                if b is None:
                    continue
                try:
                    ch, vs = int(parts[1]), int(parts[2])
                except ValueError:
                    continue
                en = en.replace("<em>", "").replace("</em>", "")
                order = 0
                for raw_word in en.split():
                    tags = STRONG_TAG.findall(raw_word)
                    surface = STRONG_TAG.sub("", raw_word).strip().strip(",.;:!?")
                    if not tags:
                        if surface:
                            rows.append([b, ch, vs, order, surface, None, None, None, None, None, "stepbible-tagnt"])
                            order += 1
                    else:
                        for t in tags:
                            rows.append([b, ch, vs, order, surface, t, None, t, None, None, "stepbible-tagnt"])
                            order += 1
    # Optional: STEPBible morphological TSV (book, chapter, verse, surface, strongs,
    # morphology, extended, lemma, gloss).
    for src_name, source_id in (("stepbible-tagnt.tsv", "stepbible-tagnt"), ("stepbible-tahot.tsv", "stepbible-tahot")):
        path = os.path.join(raw, src_name)
        if not os.path.exists(path):
            continue
        for line in open(path, encoding="utf-8"):
            f = [c.strip() for c in line.split("\t")]
            if len(f) < 9:
                continue
            b = norm_book(f[0])
            try:
                ch, vs = int(f[1]), int(f[2])
            except ValueError:
                warn(f"verse_words: bad reference {f[1]}:{f[2]}")
                continue
            if b is None or ch < 1 or vs < 1:
                warn(f"verse_words: rejected row")
                continue
            rows.append([b, ch, vs, 0, f[3], f[4] or None, f[5] or None, f[6] or None, f[7] or None, f[8] or None, source_id])
    # Stabilize word order per verse.
    seen = {}
    for row in rows:
        key = (row[0], row[1], row[2])
        row[3] = seen.get(key, 0)
        seen[key] = row[3] + 1
    rows.sort(key=lambda r: (r[0], r[1], r[2], r[3]))
    return rows


def etl_xrefs(raw, warn):
    src = os.path.join(raw, "cross_references.txt")
    if not os.path.exists(src):
        return []
    rows = []
    with open(src, encoding="utf-8") as f:
        for line in f:
            if line.startswith("From Verse") or not line.strip():
                continue
            cols = line.rstrip("\n").split("\t")
            if len(cols) < 3:
                continue
            from_list = parse_ref_range(cols[0])
            to_list = parse_ref_range(cols[1])
            if not from_list or not to_list:
                warn(f"xrefs: rejected reference {cols[0]} -> {cols[1]}")
                continue
            try:
                rank = int(cols[2])
            except ValueError:
                rank = 1
            for fr in from_list:
                for to in to_list:
                    rows.append([fr[0], fr[1], fr[2], to[0], to[1], to[2], rank, rank, "openbible-xrefs"])
    rows.sort(key=lambda r: (r[0], r[1], r[2], r[3], r[4], r[5]))
    return rows


def etl_topics(raw, warn, source_id, version):
    path = os.path.join(raw, f"{source_id}.tsv")
    if not os.path.exists(path):
        return [], []
    topics = {}
    refs = []
    for line in open(path, encoding="utf-8"):
        f = [c.strip() for c in line.split("\t")]
        if len(f) < 4:
            continue
        name = norm_text(f[0])
        b = norm_book(f[1])
        try:
            ch, vs = int(f[2]), int(f[3])
        except ValueError:
            warn(f"topics: bad reference {f[2]}:{f[3]}")
            continue
        if not name or b is None or ch < 1 or vs < 1:
            warn(f"topics: rejected row {f[:4]!r}")
            continue
        topic_id = f"{source_id}::{re.sub(r'[^a-z0-9]+', '-', name.lower()).strip('-')}"
        topics[topic_id] = name
        refs.append((topic_id, b, ch, vs))
    topic_rows = sorted((tid, topics[tid], source_id, version) for tid in topics)
    topic_verse_rows = sorted(set(refs))
    return topic_rows, [(tid, b, c, v, 1.0, source_id) for (tid, b, c, v) in topic_verse_rows]


# ── Driver + manifest ─────────────────────────────────────────────────────────

def write_tsv(path, rows):
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        for row in rows:
            f.write("\t".join("" if c is None else str(c) for c in row) + "\n")


def run(raw_dir, clean_dir):
    os.makedirs(clean_dir, exist_ok=True)
    warnings = []
    warn = warnings.append

    outputs = {}
    raw_checksums = {}

    def ingest(name, rows, out_name):
        if rows:
            outputs[out_name] = rows
        raw_path = os.path.join(raw_dir, name)
        if os.path.exists(raw_path):
            raw_checksums[name] = sha256(raw_path)

    ingest("t_kjv.csv", etl_verses(raw_dir, warn), "verses.tsv")
    ingest("strongs-greek.js", etl_lexicon(raw_dir, warn), "lexicon.tsv")
    ingest("kjv_strongs", etl_verse_words(raw_dir, warn), "verse_words.tsv")
    ingest("cross_references.txt", etl_xrefs(raw_dir, warn), "xrefs.tsv")

    naves_t, naves_tv = etl_topics(raw_dir, warn, "naves-topical", "1896")
    torrey_t, torrey_tv = etl_topics(raw_dir, warn, "torrey-topical", "1897")
    topics = sorted(naves_t + torrey_t)
    topic_verses = sorted(naves_tv + torrey_tv)
    if topics:
        outputs["topics.tsv"] = topics
    if topic_verses:
        outputs["topic_verses.tsv"] = topic_verses

    row_counts = {}
    output_checksums = {}
    for out_name, rows in outputs.items():
        path = os.path.join(clean_dir, out_name)
        write_tsv(path, rows)
        row_counts[out_name] = len(rows)
        output_checksums[out_name] = sha256(path)

    manifest = {
        "etl_version": ETL_VERSION,
        "raw_checksums": dict(sorted(raw_checksums.items())),
        "output_checksums": dict(sorted(output_checksums.items())),
        "row_counts": dict(sorted(row_counts.items())),
        "warnings": list(warnings),
        "errors": [],
    }
    with open(os.path.join(clean_dir, "etl-manifest.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, sort_keys=True)
        f.write("\n")
    return manifest


# ── Self-test: deterministic on a tiny bundled fixture ────────────────────────

_FIXTURES = {
    "t_kjv.csv": "id,b,c,v,t\n1003001,43,3,16,For God so loved the world.\n1003028,45,8,28,And we know.\n",
    "strongs-greek.js": "{\"G26\": {\"lemma\": \"agape\", \"translit\": \"agape\", \"pron\": \"ag-ah'-pay\", \"part_of_speech\": \"n f\", \"strongs_def\": \"love\", \"kjv_def\": \"love\", \"derivation\": \"from G25\"}}\n",
    "cross_references.txt": "From Verse\tTo Verse\tVotes\t#www.openbible.info\nJohn.3.16\tRom.8.28\t50\n",
    "naves-topical.tsv": "Love\tJohn\t3\t16\nLove\tRom\t8\t28\n",
}


def self_test(tmp_dir):
    raw = os.path.join(tmp_dir, "raw")
    os.makedirs(raw, exist_ok=True)
    for name, content in _FIXTURES.items():
        with open(os.path.join(raw, name), "w", encoding="utf-8") as f:
            f.write(content)

    m1 = run(raw, os.path.join(tmp_dir, "clean-a"))
    m2 = run(raw, os.path.join(tmp_dir, "clean-b"))
    assert m1 == m2, "ETL must be deterministic"
    assert m1["row_counts"].get("verses.tsv") == 2
    assert m1["row_counts"].get("lexicon.tsv") == 1
    assert m1["row_counts"].get("xrefs.tsv") == 1
    assert m1["row_counts"].get("topics.tsv") == 1
    assert m1["row_counts"].get("topic_verses.tsv") == 2
    with open(os.path.join(tmp_dir, "clean-a", "verses.tsv"), encoding="utf-8") as f:
        assert "43\t3\t16\tFor God so loved the world." in f.read()
    print("self-test passed:", json.dumps(m1, indent=2))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--raw", default="data/raw")
    ap.add_argument("--out", default="data/clean")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            self_test(tmp)
        return

    manifest = run(args.raw, args.out)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    if manifest["errors"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
