# Data Sources & Provenance

Sermon Studio ships no Bible data in its source tree. The canon vault is built
from public-domain and permissively licensed sources, fetched into `data/raw/`
and normalized by `tools/etl.py` into `data/clean/*.tsv`.

## Sources

| Dataset | Source | License | Rows |
| --- | --- | --- | --- |
| KJV Bible text | [scrollmapper/bible_databases](https://github.com/scrollmapper/bible_databases) | Public domain | 31,102 verses |
| Strong's Greek & Hebrew lexicons | [openscriptures/strongs](https://github.com/openscriptures/strongs) | Public domain | 14,197 entries |
| Strong's-tagged KJV (interlinear) | [kaiserlik/kjv](https://github.com/kaiserlik/kjv) | Public domain | 2,774,190 verse-words |
| Cross references (TSK / OpenBible) | [OpenBible.info](https://www.openbible.info/labs/cross-references/) | CC-BY | 256,648 links |

## Normalization (`tools/etl.py`)

The raw sources are inconsistent in ways that matter:

* **Book naming.** The KJV CSV uses Roman numerals ("I Chronicles", "III John")
  and "Revelation of John". The ETL maps these to canonical names via an alias
  table, which is why the verse count reaches the full 31,102 rather than the
  24,570 a naive import yields.
* **Malformed JSON.** The Strong's-tagged KJV files contain unescaped quotes in
  non-English fields (notably `1Co.json`). The ETL uses a tolerant regex
  extractor rather than a strict JSON parser so no verses are dropped.
* **Field shapes.** Lexicon entries vary between Greek and Hebrew; the ETL
  normalizes them to a single 9-column TSV.

## Output TSVs

| File | Columns |
| --- | --- |
| `verses.tsv` | book_num, chapter, verse, text_kjv |
| `verse_words.tsv` | book_num, chapter, verse, word_order, surface_word, strong_id, morphology |
| `lexicon.tsv` | strong_id, testament, lemma, transliteration, pronunciation, part_of_speech, definition, gloss, derivation |
| `xrefs.tsv` | from_book, from_chapter, from_verse, to_book, to_chapter, to_verse, rank |

## Rebuilding the canon

```bash
scripts/build-canon.sh [RAW_DIR] [OUT_DB]
```

The builder is idempotent: it recreates the canon tables from scratch, so it can
be re-run at any time. The resulting `canon.db` is read-only at run time.

## Attribution

Cross-reference data is provided by OpenBible.info under CC-BY. If you
redistribute `canon.db`, retain this attribution. The KJV text and Strong's
lexicons are public domain.
