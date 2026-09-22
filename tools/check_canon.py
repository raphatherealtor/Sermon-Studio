import sqlite3, json

c = sqlite3.connect('data/canon.db')

print('John 3:16 =', c.execute(
    "SELECT text_kjv FROM bible_verses WHERE book_num=43 AND chapter=3 AND verse=16"
).fetchone()[0])

print('books  =', c.execute('SELECT COUNT(*) FROM bible_books').fetchone()[0])
print('verses =', c.execute('SELECT COUNT(*) FROM bible_verses').fetchone()[0])
print('xrefs  =', c.execute('SELECT COUNT(*) FROM cross_references').fetchone()[0])
print('sources=', c.execute('SELECT COUNT(*) FROM sources').fetchone()[0])

print('Gen 1:1 -> xrefs =', c.execute(
    "SELECT COUNT(*) FROM cross_references cr JOIN bible_verses f ON cr.from_verse_id=f.id "
    "WHERE f.book_num=1 AND f.chapter=1 AND f.verse=1"
).fetchone()[0])

m = c.execute("SELECT value FROM canon_meta WHERE key='manifest'").fetchone()
if m:
    manifest = json.loads(m[0])
    print('manifest canon_version =', manifest['canon_version'])
    print('manifest sources      =', len(manifest['source_versions']))
    print('manifest has kjv-pd   =', 'kjv-pd' in manifest['source_checksums'])
    print('manifest has openbible=', 'openbible-xrefs' in manifest['source_checksums'])

# A concrete xref evidence: Gen 1:1 -> Isa 65:17
row = c.execute(
    "SELECT f.book_num,f.chapter,f.verse,t.book_num,t.chapter,t.verse,cr.rank "
    "FROM cross_references cr JOIN bible_verses f ON cr.from_verse_id=f.id "
    "JOIN bible_verses t ON cr.to_verse_id=t.id "
    "WHERE f.book_num=1 AND f.chapter=1 AND f.verse=1 AND t.book_num=23 AND t.chapter=65 AND t.verse=17"
).fetchone()
print('Gen1:1 -> Isa65:17 xref =', row)

c.close()
