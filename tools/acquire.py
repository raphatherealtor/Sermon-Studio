#!/usr/bin/env python3
"""Sermon Studio — offline source acquisition (BUILD-TIME ONLY).

Downloads the raw public-domain / openly-licensed datasets into data/raw/ so
tools/etl.py can normalize them. Runtime never downloads: this script is a
one-time, offline, human-run build step. Each source is pinned to a URL + SHA-256
for provenance.

Acquired:
  * KJV text               — t_kjv.csv (bible_databases, 31,103 verses)
  * OpenBible cross-refs   — cross_references.txt (CC-BY, ranges expanded by ETL)
  * Strong's Greek/Hebrew  — OpenScriptures strongs-greek/hebrew-dictionary.js
  * STEPBible TAGNT/TAHOT  — Tyndale/STEPBible tagged Greek NT + Hebrew OT
  * STEPBible TBESH/TBESG  — Brief lexicon of Extended Strongs (Hebrew/Greek)
  * Nave's Topical Bible   — audited normalized assertions (j86schroeder)
  * Torrey's New Topical Textbook — audited normalized assertions (j86schroeder)

Usage:  python3 tools/acquire.py [--out data/raw]
"""
import argparse
import hashlib
import json
import os
import sys
import urllib.parse
import urllib.request

_STEP = "https://raw.githubusercontent.com/STEPBible/STEPBible-Data/master/"
_OS = "https://raw.githubusercontent.com/openscriptures/strongs/master/"
_TOPICAL = "https://raw.githubusercontent.com/j86schroeder/topical-bible-search/main/"


def _step(path):
    return _STEP + urllib.parse.quote(path)


# filename -> (url, attribution, license_code)
SOURCES = {
    "t_kjv.csv": (
        "https://raw.githubusercontent.com/xjlin0/bible_databases/master/csv/t_kjv.csv",
        "King James Version (public domain in the United States).",
        "PD",
    ),
    "cross_references.txt": (
        "https://raw.githubusercontent.com/xjlin0/bible_databases/master/cross_references.txt",
        "OpenBible.info cross-references, CC BY 4.0.",
        "CC-BY-4.0",
    ),
    "strongs-greek.js": (
        _OS + "greek/strongs-greek-dictionary.js",
        "Open Scriptures Strong's Greek dictionary (1890 text, public domain; JSON encoding CC-BY-SA).",
        "CC-BY-SA",
    ),
    "strongs-hebrew.js": (
        _OS + "hebrew/strongs-hebrew-dictionary.js",
        "Open Scriptures Strong's Hebrew dictionary (1894 text, public domain; JSON encoding CC-BY-SA).",
        "CC-BY-SA",
    ),
    "stepbible-tbesh.tsv": (
        _step("Lexicons/TBESH - Translators Brief lexicon of Extended Strongs for Hebrew - STEPBible.org CC BY.txt"),
        "STEPBible TBESH (Tyndale House, Cambridge), CC BY 4.0.",
        "CC-BY-4.0",
    ),
    "stepbible-tbesg.tsv": (
        _step("Lexicons/TBESG - Translators Brief lexicon of Extended Strongs for Greek - STEPBible.org CC BY.txt"),
        "STEPBible TBESG (Tyndale House, Cambridge), CC BY 4.0.",
        "CC-BY-4.0",
    ),
    "naves-topics.jsonl": (
        _TOPICAL + "dist/nave/topics.jsonl",
        "Nave's Topical Bible (1896, public domain), audited normalized extraction by j86schroeder/topical-bible-search.",
        "PD",
    ),
    "naves-assertions.jsonl": (
        _TOPICAL + "dist/nave/assertions.jsonl",
        "Nave's Topical Bible (1896, public domain), audited normalized extraction by j86schroeder/topical-bible-search.",
        "PD",
    ),
    "torrey-topics.jsonl": (
        _TOPICAL + "dist/torrey/topics.jsonl",
        "Torrey's New Topical Textbook (1897, public domain), audited normalized extraction by j86schroeder/topical-bible-search.",
        "PD",
    ),
    "torrey-assertions.jsonl": (
        _TOPICAL + "dist/torrey/assertions.jsonl",
        "Torrey's New Topical Textbook (1897, public domain), audited normalized extraction by j86schroeder/topical-bible-search.",
        "PD",
    ),
}

# Multi-part sources: concatenated in order into one canonical raw file.
MULTIPART = {
    "stepbible-tagnt.tsv": (
        "STEPBible TAGNT (Tyndale House, Cambridge), CC BY 4.0.",
        "CC-BY-4.0",
        [
            _step("Translators Amalgamated OT+NT/TAGNT Mat-Jhn - Translators Amalgamated Greek NT - STEPBible.org CC-BY.txt"),
            _step("Translators Amalgamated OT+NT/TAGNT Act-Rev - Translators Amalgamated Greek NT - STEPBible.org CC-BY.txt"),
        ],
    ),
    "stepbible-tahot.tsv": (
        "STEPBible TAHOT (Tyndale House, Cambridge), CC BY 4.0.",
        "CC-BY-4.0",
        [
            _step("Translators Amalgamated OT+NT/TAHOT Gen-Deu - Translators Amalgamated Hebrew OT - STEPBible.org CC BY.txt"),
            _step("Translators Amalgamated OT+NT/TAHOT Jos-Est - Translators Amalgamated Hebrew OT - STEPBible.org CC BY.txt"),
            _step("Translators Amalgamated OT+NT/TAHOT Job-Sng - Translators Amalgamated Hebrew OT - STEPBible.org CC BY.txt"),
            _step("Translators Amalgamated OT+NT/TAHOT Isa-Mal - Translators Amalgamated Hebrew OT - STEPBible.org CC BY.txt"),
        ],
    ),
}


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _fetch(url, dst):
    req = urllib.request.Request(url, headers={"User-Agent": "sermon-studio-build"})
    with urllib.request.urlopen(req, timeout=300) as resp, open(dst, "wb") as f:
        f.write(resp.read())


def acquire(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    manifest = {}

    # Single-file sources.
    for name, (url, attribution, license_code) in sorted(SOURCES.items()):
        dst = os.path.join(out_dir, name)
        if not os.path.exists(dst) or os.path.getsize(dst) == 0:
            print(f"downloading {name} <- {url}", file=sys.stderr)
            _fetch(url, dst)
        manifest[name] = _record(name, url, attribution, license_code, dst)

    # Multi-part sources (concatenate the parts in order).
    for name, (attribution, license_code, urls) in sorted(MULTIPART.items()):
        dst = os.path.join(out_dir, name)
        if not os.path.exists(dst) or os.path.getsize(dst) == 0:
            parts = []
            for i, url in enumerate(urls):
                part = os.path.join(out_dir, f".part-{name}.{i}")
                print(f"downloading part {i} of {name} <- {url}", file=sys.stderr)
                _fetch(url, part)
                parts.append(part)
            with open(dst, "wb") as out:
                for part in parts:
                    with open(part, "rb") as f:
                        out.write(f.read())
                    out.write(b"\n")
                    os.remove(part)
        manifest[name] = _record(name, "; ".join(urls), attribution, license_code, dst)

    with open(os.path.join(out_dir, "sources.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, sort_keys=True)
        f.write("\n")
    return manifest


def _record(name, url, attribution, license_code, dst):
    rec = {
        "url": url,
        "attribution": attribution,
        "license_code": license_code,
        "sha256": sha256(dst),
        "bytes": os.path.getsize(dst),
    }
    print(f"{name}: {rec['bytes']} bytes sha256={rec['sha256']} ({license_code})")
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="data/raw")
    args = ap.parse_args()
    acquire(args.out)


if __name__ == "__main__":
    main()
