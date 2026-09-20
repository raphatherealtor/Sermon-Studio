#!/usr/bin/env python3
"""
Sermon Studio — raw data ETL.

Normalizes heterogeneous public-domain source files into clean, canonical TSVs
that the Rust `canon.db` builder ingests. This keeps the Rust side simple and
deterministic while absorbing the messiness of real-world datasets:

  * KJV.csv                -> clean/verses.tsv
  * kjv_strongs/*.json     -> clean/verse_words.tsv   (tolerant of invalid JSON)
  * strongs-{greek,hebrew}.js -> clean/lexicon.tsv
  * cross_references.txt   -> clean/xrefs.tsv

Run:  python3 tools/etl.py --raw data/raw --out data/clean
"""
import argparse
import json
import os
import re
import sys

# Canonical 66 books: (book_num, osis, display_name, testament)
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

# Extra aliases seen in the wild (lowercased, alnum-only key -> book_num).
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

# Build lookup: normalized name -> book_num
NAME_TO_NUM = {}
for num, osis, name, _t in BOOKS:
    for key in (osis, name):
        NAME_TO_NUM[re.sub(r"[^a-z0-9]", "", key.lower())] = num
for k, v in ALIASES.items():
    NAME_TO_NUM[re.sub(r"[^a-z0-9]", "", k.lower())] = v


def norm_book(name):
    key = re.sub(r"[^a-z0-9]", "", name.lower())
    return NAME_TO_NUM.get(key)


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


def etl_verses(raw, out):
    src = os.path.join(raw, "KJV.csv")
    dst = os.path.join(out, "verses.tsv")
    n = 0
    with open(src, encoding="utf-8") as f, open(dst, "w", encoding="utf-8") as w:
        next(f, None)
        for line in f:
            if not line.strip():
                continue
            fld = parse_csv_line(line)
            if len(fld) < 4:
                continue
            b = norm_book(fld[0])
            if b is None:
                continue
            try:
                ch, vs = int(fld[1]), int(fld[2])
            except ValueError:
                continue
            text = fld[3].strip().replace("\t", " ")
            w.write(f"{b}\t{ch}\t{vs}\t{text}\n")
            n += 1
    print(f"verses.tsv: {n} rows")


STRONG_TAG = re.compile(r"\[([HG]\d+)\]")
VERSE_KEY = re.compile(r'"([^"]+\|[^"]+\|[^"]+)"\s*:\s*\{\s*"en"\s*:\s*"((?:[^"\\]|\\.)*)"')


def etl_verse_words(raw, out):
    src_dir = os.path.join(raw, "kjv_strongs")
    dst = os.path.join(out, "verse_words.tsv")
    n = 0
    with open(dst, "w", encoding="utf-8") as w:
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
                try:
                    en = json.loads('"' + en + '"')
                except Exception:
                    en = en.replace('\\"', '"')
                en = en.replace("<em>", "").replace("</em>", "")
                order = 0
                for raw_word in en.split():
                    tags = STRONG_TAG.findall(raw_word)
                    surface = STRONG_TAG.sub("", raw_word).strip().strip(",.;:!?")
                    if not tags:
                        if surface:
                            w.write(f"{b}\t{ch}\t{vs}\t{order}\t{surface}\t\t\n")
                            order += 1
                    else:
                        for t in tags:
                            w.write(f"{b}\t{ch}\t{vs}\t{order}\t{surface}\t{t}\t\n")
                            order += 1
                n += 1
    print(f"verse_words.tsv: {n} verses tokenized")


def etl_lexicon(raw, out):
    dst = os.path.join(out, "lexicon.tsv")
    n = 0
    with open(dst, "w", encoding="utf-8") as w:
        for fn, testament in (("strongs-hebrew.js", "OT"), ("strongs-greek.js", "NT")):
            path = os.path.join(raw, fn)
            if not os.path.exists(path):
                continue
            text = open(path, encoding="utf-8").read()
            start, end = text.find("{"), text.rfind("}")
            data = json.loads(text[start:end + 1])
            for sid, v in data.items():
                lemma = v.get("lemma", "")
                translit = v.get("translit") or v.get("xlit") or ""
                pron = v.get("pron", "")
                pos = v.get("part_of_speech", "")
                definition = v.get("strongs_def", "")
                gloss = v.get("kjv_def", "")
                deriv = v.get("derivation", "")
                row = [sid, testament, lemma, translit, pron, pos, definition, gloss, deriv]
                row = [str(x).replace("\t", " ").replace("\n", " ") for x in row]
                w.write("\t".join(row) + "\n")
                n += 1
    print(f"lexicon.tsv: {n} entries")


def etl_xrefs(raw, out):
    src = os.path.join(raw, "cross_references.txt")
    dst = os.path.join(out, "xrefs.tsv")
    n = 0
    with open(src, encoding="utf-8") as f, open(dst, "w", encoding="utf-8") as w:
        for line in f:
            if line.startswith("From Verse") or not line.strip():
                continue
            cols = line.rstrip("\n").split("\t")
            if len(cols) < 3:
                continue
            fr = parse_ref(cols[0])
            to = parse_ref(cols[1])
            if not fr or not to:
                continue
            try:
                rank = int(cols[2])
            except ValueError:
                rank = 1
            w.write(f"{fr[0]}\t{fr[1]}\t{fr[2]}\t{to[0]}\t{to[1]}\t{to[2]}\t{rank}\n")
            n += 1
    print(f"xrefs.tsv: {n} rows")


REF_RE = re.compile(r"^([1-3]?[A-Za-z]+)\.(\d+)\.(\d+)$")


def parse_ref(s):
    m = REF_RE.match(s.strip())
    if not m:
        return None
    b = norm_book(m.group(1))
    if b is None:
        return None
    return (b, int(m.group(2)), int(m.group(3)))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--raw", default="data/raw")
    ap.add_argument("--out", default="data/clean")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    etl_verses(args.raw, args.out)
    etl_verse_words(args.raw, args.out)
    etl_lexicon(args.raw, args.out)
    etl_xrefs(args.raw, args.out)


if __name__ == "__main__":
    main()
