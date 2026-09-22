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

ETL_VERSION = "2025.01-track-m-full"

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

# STEPBible's own UBS abbreviation scheme (used by TAGNT/TAHOT). Kept separate
# from NAME_TO_NUM because several collide with OpenBible/OSIS abbreviations
# (e.g. STEPBible "Jud" = Jude, "Jdg" = Judges; "1Jn" = 1 John).
STEP_BOOKS = {
    "gen": 1, "exo": 2, "lev": 3, "num": 4, "deu": 5, "jos": 6, "jdg": 7, "rut": 8,
    "1sa": 9, "2sa": 10, "1ki": 11, "2ki": 12, "1ch": 13, "2ch": 14, "ezr": 15,
    "neh": 16, "est": 17, "job": 18, "psa": 19, "pro": 20, "ecc": 21, "sng": 22,
    "isa": 23, "jer": 24, "lam": 25, "ezk": 26, "dan": 27, "hos": 28, "jol": 29,
    "amo": 30, "oba": 31, "jon": 32, "mic": 33, "nam": 34, "hab": 35, "zep": 36,
    "hag": 37, "zec": 38, "mal": 39,
    "mat": 40, "mrk": 41, "luk": 42, "jhn": 43, "act": 44, "rom": 45, "1co": 46,
    "2co": 47, "gal": 48, "eph": 49, "php": 50, "col": 51, "1th": 52, "2th": 53,
    "1ti": 54, "2ti": 55, "tit": 56, "phm": 57, "heb": 58, "jas": 59, "1pe": 60,
    "2pe": 61, "1jn": 62, "2jn": 63, "3jn": 64, "jud": 65, "rev": 66,
}


def _int(x, default=0):
    return int(x) if x is not None else default


def norm_book(name):
    return NAME_TO_NUM.get(re.sub(r"[^a-z0-9]", "", name.lower()))


def norm_text(s):
    return " ".join(unicodedata.normalize("NFC", s or "").split())


def strip_html(s):
    return re.sub(r"<[^>]*>", " ", s or "")


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


def classic_strongs(token):
    """Backward-compatible Strong's number: strip zero-padding, instance suffix
    (`_A`), and STEPBible disambiguation letters (`H7225G` -> `H7225`)."""
    m = re.search(r"([HG])(\d+)", token or "")
    if not m:
        return None
    return m.group(1) + str(int(m.group(2)))


# STEPBible word/verse reference used by TAGNT and TAHOT: `Mat.1.1#01=NKO`.
STEP_REF_RE = re.compile(r"^([0-9]?[A-Za-z]+)\.(\d+)\.(\d+)#(\d+)=(\S+)$")


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
            text = norm_text(fld[4])
            # Empty/placeholder verse slots (e.g. the source's non-canonical
            # 3 John 1:15 emitted as "[]") are versification artifacts, not verses.
            if text in ("", "[]", "()"):
                warn(f"verses: dropped empty verse {b}:{c}:{v}")
                continue
            rows.append((b, c, v, text))
    rows.sort(key=lambda r: (r[0], r[1], r[2]))
    return rows


def etl_lexicon(raw, warn):
    # Strong's (OpenScriptures) first — the authoritative classic lexicon.
    strongs = {}
    for fn, testament in (("strongs-hebrew.js", "OT"), ("strongs-greek.js", "NT")):
        path = os.path.join(raw, fn)
        if not os.path.exists(path):
            continue
        text = open(path, encoding="utf-8").read()
        data = json.loads(text[text.find("{"):text.rfind("}") + 1])
        for sid, v in data.items():
            strongs[sid] = [
                sid, testament,
                norm_text(v.get("lemma", "")),
                norm_text(v.get("translit") or v.get("xlit") or ""),
                norm_text(v.get("pron", "")) or None,
                norm_text(v.get("part_of_speech", "")) or None,
                norm_text(v.get("strongs_def", "")),
                norm_text(v.get("kjv_def", "")),
                norm_text(v.get("derivation", "")) or None,
                None, "strongs-pd", "1890",
            ]

    # STEPBible brief lexicons: gap-fill extended ids beyond classic Strong's.
    # (Strong's full definitions win for the classic id space; the disambiguated
    # letter-suffixed ids are NOT stored because they would fail the malformed-id
    # gate.)
    emitted = set(strongs.keys())
    rows = list(strongs.values())
    for fn, testament, source_id, version in (
        ("stepbible-tbesh.tsv", "OT", "stepbible-tbesh", "tbesh"),
        ("stepbible-tbesg.tsv", "NT", "stepbible-tbesg", "tbesg"),
    ):
        path = os.path.join(raw, fn)
        if not os.path.exists(path):
            continue
        for line in open(path, encoding="utf-8-sig"):
            f = line.rstrip("\n").split("\t")
            if len(f) < 8:
                continue
            m = re.match(r"^([HG])(\d+)$", f[0].strip())
            if not m:
                continue
            sid = m.group(1) + str(int(m.group(2)))  # unpad: H0001 -> H1
            if sid in emitted:
                continue
            emitted.add(sid)
            rows.append([
                sid, testament,
                norm_text(f[3]),                 # lemma
                norm_text(f[4]),                 # transliteration
                None,                            # pronunciation (not provided)
                norm_text(f[5]) or None,         # part_of_speech (e.g. H:N-M)
                norm_text(strip_html(f[7])),     # definition (HTML stripped)
                norm_text(f[6]),                 # gloss
                None,                            # derivation
                None,                            # usage_note
                source_id, version,
            ])
    rows.sort(key=lambda r: r[0])
    return rows


def _tahot_lemma_gloss(col11, root):
    """Extract lemma + gloss for the root strong's from the `Expanded Strong tags`
    column, e.g. `H9003=ב=in/{H7225G=רֵאשִׁית=: beginning»first:1_beginning}`."""
    if not root:
        return None, None
    for m in re.finditer(r"\{([HG]\d+[A-Za-z]?)=([^=]+)=([^»}=]*)", col11 or ""):
        if m.group(1).lower() == root.lower():
            return m.group(2), m.group(3).lstrip(": ")
    return None, None


def etl_verse_words(raw, warn):
    rows = []

    # TAGNT — Greek NT (NA27/28 base reading: edition flag contains uppercase N).
    path = os.path.join(raw, "stepbible-tagnt.tsv")
    if os.path.exists(path):
        for line in open(path, encoding="utf-8-sig"):
            f = line.rstrip("\n").split("\t")
            if len(f) < 12:
                continue
            m = STEP_REF_RE.match(f[0].strip())
            if not m:
                continue
            b = STEP_BOOKS.get(m.group(1).lower())
            if b is None or "N" not in m.group(5):
                continue
            ch, vs = int(m.group(2)), int(m.group(3))
            order = int(m.group(4)) - 1
            surface = norm_text(re.sub(r"\s*\([^)]*\)\s*$", "", f[1]))
            dstrong, _, grammar = f[3].strip().partition("=")
            lemma, _, gloss = f[4].strip().partition("=")
            rows.append([
                b, ch, vs, order,
                surface,
                classic_strongs(f[11]),
                norm_text(grammar) or None,
                norm_text(dstrong) or None,
                norm_text(lemma) or None,
                norm_text(gloss) or None,
                "stepbible-tagnt",
            ])

    # TAHOT — Hebrew OT (Leningrad base reading: flag == "L").
    path = os.path.join(raw, "stepbible-tahot.tsv")
    if os.path.exists(path):
        for line in open(path, encoding="utf-8-sig"):
            f = line.rstrip("\n").split("\t")
            if len(f) < 12:
                continue
            m = STEP_REF_RE.match(f[0].strip())
            if not m:
                continue
            b = STEP_BOOKS.get(m.group(1).lower())
            if b is None or m.group(5) != "L":
                continue
            ch, vs = int(m.group(2)), int(m.group(3))
            order = int(m.group(4)) - 1
            surface = norm_text(f[1])
            grammar = f[5].strip()
            root = re.sub(r"_[A-Za-z]*$", "", f[8].strip())  # Root dStrong, strip instance
            lemma, gloss = _tahot_lemma_gloss(f[11], root)
            rows.append([
                b, ch, vs, order,
                surface,
                classic_strongs(root),
                norm_text(grammar) or None,
                norm_text(root) or None,
                norm_text(lemma) or None,
                norm_text(gloss) or None,
                "stepbible-tahot",
            ])

    # Dedupe by (book, chapter, verse, word_order) and stabilize order.
    seen = set()
    deduped = []
    for r in rows:
        key = (r[0], r[1], r[2], r[3])
        if key in seen:
            continue
        seen.add(key)
        deduped.append(r)
    deduped.sort(key=lambda r: (r[0], r[1], r[2], r[3]))
    return deduped


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


def _expand_ref(b, c1, v1, c2, v2, chapter_max):
    """Expand (c1,v1)..(c2,v2) into individual (book, chapter, verse) tuples using
    real KJV chapter boundaries. Returns an empty list if the start is unknown."""
    if (b, c1) not in chapter_max:
        return []
    out = []
    c, v = c1, v1
    while (c, v) <= (c2, v2):
        out.append((b, c, v))
        maxv = chapter_max.get((b, c))
        if maxv is None:
            break
        if v < maxv:
            v += 1
        else:
            c += 1
            v = 1
        if len(out) > 500:
            break
    return out


def etl_topics(raw, warn, source_id, version, topics_fn, assertions_fn, valid_verses, chapter_max):
    """Normalize a topical source (topics.jsonl + assertions.jsonl) into
    `topics.tsv` rows and `topic_verses.tsv` rows (topic -> one verse each)."""
    topics = {}
    topics_path = os.path.join(raw, topics_fn)
    if os.path.exists(topics_path):
        for line in open(topics_path, encoding="utf-8"):
            line = line.strip()
            if not line:
                continue
            d = json.loads(line)
            tid = d.get("id")
            if tid:
                topics[tid] = norm_text(d.get("sourceTopic", tid))

    refs = []
    dropped = 0
    assertions_path = os.path.join(raw, assertions_fn)
    if os.path.exists(assertions_path):
        for line in open(assertions_path, encoding="utf-8"):
            line = line.strip()
            if not line:
                continue
            d = json.loads(line)
            if d.get("sourceStatus") != "source_valid":
                continue
            tid = d.get("topicId")
            if not tid:
                continue
            b = norm_book(d.get("book", ""))
            if b is None:
                dropped += 1
                continue
            topics.setdefault(tid, norm_text(d.get("sourceTopic", tid)))
            c1 = _int(d.get("chapterStart"), 1)
            if d.get("scope") == "chapter":
                v1, c2, v2 = 1, c1, chapter_max.get((b, c1), 1)
            else:
                v1 = _int(d.get("verseStart"), 1)
                c2 = _int(d.get("chapterEnd"), c1)
                v2 = _int(d.get("verseEnd"), v1)
            for (bb, cc, vv) in _expand_ref(b, c1, v1, c2, v2, chapter_max):
                if (bb, cc, vv) in valid_verses:
                    refs.append((tid, bb, cc, vv))
                else:
                    dropped += 1
    if dropped:
        warn(f"topics({source_id}): {dropped} references rejected (unknown verse/chapter)")

    topic_rows = sorted((tid, topics[tid], source_id, version) for tid in topics)
    topic_verse_rows = sorted((tid, b, c, v, 1.0, source_id) for (tid, b, c, v) in sorted(set(refs)))
    return topic_rows, topic_verse_rows


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

    def checksum_raw(name):
        p = os.path.join(raw_dir, name)
        if os.path.exists(p):
            raw_checksums[name] = sha256(p)

    def put(out_name, rows):
        if rows:
            outputs[out_name] = rows

    verses = etl_verses(raw_dir, warn)
    checksum_raw("t_kjv.csv")
    put("verses.tsv", verses)

    valid_verses = {(b, c, v) for (b, c, v, _t) in verses}
    chapter_max = {}
    for (b, c, v, _t) in verses:
        chapter_max[(b, c)] = max(chapter_max.get((b, c), 0), v)

    lexicon = etl_lexicon(raw_dir, warn)
    for n in ("strongs-greek.js", "strongs-hebrew.js", "stepbible-tbesh.tsv", "stepbible-tbesg.tsv"):
        checksum_raw(n)
    put("lexicon.tsv", lexicon)

    verse_words = etl_verse_words(raw_dir, warn)
    for n in ("stepbible-tagnt.tsv", "stepbible-tahot.tsv"):
        checksum_raw(n)
    put("verse_words.tsv", verse_words)

    xrefs = etl_xrefs(raw_dir, warn)
    checksum_raw("cross_references.txt")
    put("xrefs.tsv", xrefs)

    naves_t, naves_tv = etl_topics(
        raw_dir, warn, "naves-topical", "1896",
        "naves-topics.jsonl", "naves-assertions.jsonl", valid_verses, chapter_max,
    )
    torrey_t, torrey_tv = etl_topics(
        raw_dir, warn, "torrey-topical", "1897",
        "torrey-topics.jsonl", "torrey-assertions.jsonl", valid_verses, chapter_max,
    )
    for n in ("naves-topics.jsonl", "naves-assertions.jsonl", "torrey-topics.jsonl", "torrey-assertions.jsonl"):
        checksum_raw(n)
    topics = sorted(naves_t + torrey_t)
    topic_verses = sorted(naves_tv + torrey_tv)
    put("topics.tsv", topics)
    put("topic_verses.tsv", topic_verses)

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
    "t_kjv.csv": "id,b,c,v,t\n1003001,43,3,16,For God so loved the world.\n1003028,45,8,28,And we know.\n1003064,64,1,15,[]\n",
    "strongs-greek.js": '{"G26": {"lemma": "agape", "translit": "agape", "strongs_def": "love", "kjv_def": "love", "derivation": "from G25"}}\n',
    "strongs-hebrew.js": '{"H430": {"lemma": "elohim", "xlit": "elohim", "pron": "el-o-heem", "strongs_def": "God", "kjv_def": "God", "derivation": "plural of H433"}}\n',
    "stepbible-tbesh.tsv": "TBESH header\nH9001\tH9001 =\tH9001\tav\tav\tH:N-M\tfather\tfather <b>definition</b>\n",
    "stepbible-tbesg.tsv": "TBESG header\nG9999\tG9999 =\tG9999\tab\tab\tG:N\tgloss\tdefinition text\n",
    "stepbible-tagnt.tsv": "Word & Type\tGreek\tEnglish translation\tdStrongs = Grammar\tDictionary form = Gloss\teditions\tMeaning variants\tSpelling variants\tSpanish translation\tSub-meaning\tConjoin word\tsStrong+Instance\tAlt Strongs\nMat.1.1#01=NKO\tΒίβλος (Biblos)\tbook\tG0976=N-NSF\tβίβλος=book\tNA28\t\t\t\tbook\t#01\tG0976\n",
    "stepbible-tahot.tsv": "Eng (Heb) Ref & Type\tHebrew\tTransliteration\tTranslation\tdStrongs\tGrammar\tMeaning Variants\tSpelling Variants\tRoot dStrong+Instance\tAlternative Strongs+Instance\tConjoin word\tExpanded Strong tags\nGen.1.1#01=L\tרֵאשִׁית\treshit\tbeginning\t{H7225G}\tHNcfsa\t\t\tH7225G\t\t\t{H7225G=רֵאשִׁית=: beginning}\n",
    "cross_references.txt": "From Verse\tTo Verse\tVotes\t#www.openbible.info\nJohn.3.16\tRom.8.28\t50\n",
    "naves-topics.jsonl": '{"id":"nave:love","source":"nave","sourceTopic":"LOVE"}\n',
    "naves-assertions.jsonl": '{"topicId":"nave:love","source":"nave","sourceStatus":"source_valid","sourceTopic":"LOVE","book":"John","chapterStart":3,"verseStart":16,"chapterEnd":3,"verseEnd":16,"scope":"verse"}\n',
    "torrey-topics.jsonl": '{"id":"torrey:faith","source":"torrey","sourceTopic":"Faith"}\n',
    "torrey-assertions.jsonl": '{"topicId":"torrey:faith","source":"torrey","sourceStatus":"source_valid","sourceTopic":"Faith","book":"Romans","chapterStart":8,"verseStart":28,"chapterEnd":8,"verseEnd":28,"scope":"verse"}\n',
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
    assert m1["row_counts"].get("lexicon.tsv") == 4
    assert m1["row_counts"].get("verse_words.tsv") == 2
    assert m1["row_counts"].get("xrefs.tsv") == 1
    assert m1["row_counts"].get("topics.tsv") == 2
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
