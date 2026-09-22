#!/usr/bin/env python3
"""Sermon Studio — offline source acquisition (BUILD-TIME ONLY).

Downloads the raw public-domain datasets into data/raw/ so tools/etl.py can
normalize them. Runtime never downloads: this script is a one-time, offline,
human-run build step. Each source is pinned to a URL + SHA-256 for provenance.

Acquired now:
  * KJV text              — t_kjv.csv (bible_databases, 31,103 verses)
  * OpenBible cross-refs  — cross_references.txt (CC-BY, ranges expanded by ETL)

Documented (acquisition point reserved; raw formats verified separately):
  * Strong's Greek/Hebrew — OpenScriptures (openscriptures/strongs)
  * STEPBible TAGNT/TAHOT/TBESH/TBESG — github.com/STEPBible/STEPBible-Data
  * Nave's Topical Bible / Torrey's New Topical Textbook — CCEL

Usage:  python3 tools/acquire.py [--out data/raw]
"""
import argparse
import hashlib
import json
import os
import sys
import urllib.request

SOURCES = {
    "t_kjv.csv": (
        "https://raw.githubusercontent.com/xjlin0/bible_databases/master/csv/t_kjv.csv",
        "King James Version (public domain in the United States).",
    ),
    "cross_references.txt": (
        "https://raw.githubusercontent.com/xjlin0/bible_databases/master/cross_references.txt",
        "OpenBible.info cross-references, CC BY 4.0.",
    ),
}


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def acquire(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    manifest = {}
    for name, (url, attribution) in sorted(SOURCES.items()):
        dst = os.path.join(out_dir, name)
        if not os.path.exists(dst):
            print(f"downloading {name} <- {url}", file=sys.stderr)
            req = urllib.request.Request(url, headers={"User-Agent": "sermon-studio-build"})
            with urllib.request.urlopen(req) as resp, open(dst, "wb") as f:
                f.write(resp.read())
        checksum = sha256(dst)
        manifest[name] = {"url": url, "attribution": attribution, "sha256": checksum, "bytes": os.path.getsize(dst)}
        print(f"{name}: {manifest[name]['bytes']} bytes sha256={checksum}")
    with open(os.path.join(out_dir, "sources.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, sort_keys=True)
        f.write("\n")
    return manifest


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="data/raw")
    args = ap.parse_args()
    acquire(args.out)


if __name__ == "__main__":
    main()
