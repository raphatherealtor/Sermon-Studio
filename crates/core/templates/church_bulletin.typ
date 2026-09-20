// Sermon Studio — church bulletin template.
// Template id: church_bulletin — version 1.0.0 (keep in sync with export.rs).
//
// Consumes the same canonical `sermon` dict as the pulpit template, but
// renders only concise, congregation-facing outline content: no manuscript
// prose, and private exegetical notes are structurally excluded upstream
// (export.rs rejects private material for this format entirely).
// Deterministic by construction: never queries the current date, document date disabled.

// Optional-field fallback (Typst `or` is boolean-only).
#let o(x, alt) = if x != none { x } else { alt }

#let bulletin-doc(sermon) = {
  set document(
    title: sermon.meta.title,
    author: "Sermon Studio",
    date: none,
  )
  set page(
    paper: "a5",
    margin: (x: 1.5cm, y: 1.6cm),
  )
  set text(font: "Libertinus Serif", size: 10pt, lang: "en")
  set par(justify: false, leading: 0.68em, spacing: 0.95em)
  set heading(numbering: none)

  // ---- Header ------------------------------------------------------------
  [
    #set text(size: 15pt, weight: "bold")
    #(sermon.meta.title)

    #v(0.2em)
    #set text(size: 9pt, fill: luma(90))
    #o(sermon.meta.passage, "")
    #if sermon.meta.date != none [ · #(sermon.meta.date)]
    #if sermon.meta.series != none [ · #(sermon.meta.series)]

    #if sermon.meta.big_idea != none [
      #v(0.3em)
      #set text(size: 10pt, style: "italic")
      #(sermon.meta.big_idea)
    ]

    #v(0.4em)
    #line(length: 100%, stroke: 0.7pt + luma(60))
    #v(0.2em)
  ]

  // ---- Sermon outline (congregation-facing) ------------------------------
  for b in sermon.blocks {
    if b.kind == "markdown" {
      // Keep only heading lines from interstitial markdown.
      if b.outline != "" { b.outline }
    } else if b.kind == "movement" {
      let title-part = if b.title != none [ — #b.title] else []
      let head = if b.order != none [Movement #b.order#title-part] else [Movement#title-part]
      heading(level: 2, head)
      for item in b.bullets [- #item]
    } else if b.kind == "application" {
      v(0.25em)
      if b.audience != none {
        text(size: 9.5pt)[#text(weight: "semibold")[Living it out — #(b.audience)] #b.text]
      } else {
        text(size: 9.5pt)[#text(weight: "semibold")[Living it out] #b.text]
      }
      v(0.1em)
    }
  }

  v(0.6em)
  line(length: 100%, stroke: 0.4pt + luma(190))
  text(size: 8pt, fill: luma(130))[Exported from Sermon Studio.]
}
