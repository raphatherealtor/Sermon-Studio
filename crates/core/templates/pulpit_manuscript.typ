// Sermon Studio — pulpit manuscript template.
// Template id: pulpit_manuscript — version 1.0.0 (keep in sync with export.rs).
//
// Consumes the canonical `sermon` dict produced by crates/core/src/export.rs.
// Deterministic by construction: never queries the current date, document date disabled.
// Modes: "manuscript" | "outline" | "combined".

// Optional-field fallback (Typst `or` is boolean-only).
#let o(x, alt) = if x != none { x } else { alt }

#let pulpit-doc(sermon) = {
  set document(
    title: sermon.meta.title,
    author: "Sermon Studio",
    date: none,
  )
  set page(
    paper: "a4",
    margin: (x: 2.2cm, y: 2.4cm),
    numbering: "1",
    header: context {
      let page-num = counter(page).get().first()
      if page-num > 1 [
        #set text(size: 8.5pt, fill: luma(110))
        #o(sermon.meta.title, "Untitled") #h(1fr) #o(sermon.meta.passage, "")
        #v(0.35em)
        #line(length: 100%, stroke: 0.4pt + luma(190))
      ]
    },
  )
  set text(font: "Libertinus Serif", size: 11.5pt, lang: "en")
  set par(justify: true, leading: 0.72em, spacing: 1.15em)
  set heading(numbering: none)

  let mode = sermon.mode

  // ---- Title block -------------------------------------------------------
  [
    #set text(size: 17.5pt, weight: "bold")
    #(sermon.meta.title)

    #v(0.25em)
    #set text(size: 10pt, fill: luma(90))
    #o(sermon.meta.passage, "")
    #if sermon.meta.date != none [ · #(sermon.meta.date)]
    #if sermon.meta.series != none [ · #(sermon.meta.series)]
    #if sermon.meta.season != none [ · #(sermon.meta.season)]

    #if sermon.meta.big_idea != none [
      #v(0.3em)
      #set text(size: 11pt, style: "italic", fill: luma(40))
      Big Idea: #(sermon.meta.big_idea)
    ]

    #v(0.5em)
    #line(length: 100%, stroke: 0.7pt + luma(60))
    #v(0.4em)
  ]

  // ---- Body --------------------------------------------------------------
  for b in sermon.blocks {
    if b.kind == "markdown" {
      if mode == "outline" {
        if b.outline != "" { b.outline }
      } else {
        b.text
      }
    } else if b.kind == "movement" {
      let title-part = if b.title != none [ — #b.title] else []
      let head = if b.order != none [Movement #b.order#title-part] else [Movement#title-part]
      heading(head)
      if b.warrant != none [
        #text(size: 10pt, style: "italic", fill: luma(90))[Warrant: #(b.warrant)]
      ]
      if mode == "manuscript" {
        if b.manuscript != none { b.manuscript }
        if b.manuscript == none and b.bullets.len() > 0 {
          for item in b.bullets [- #item]
        }
      } else if mode == "outline" {
        for item in b.bullets [- #item]
      } else {
        if b.manuscript != none { b.manuscript }
        if b.bullets.len() > 0 {
          v(0.3em)
          text(size: 10pt, weight: "semibold", fill: luma(70))[Outline]
          for item in b.bullets [- #item]
        }
      }
    } else if b.kind == "illustration" {
      v(0.4em)
      block(
        width: 100%,
        fill: luma(97%),
        stroke: (left: 2.5pt + luma(140)),
        inset: 10pt,
        radius: (right: 3pt),
      )[
        #set text(size: 10.5pt)
        #if b.label != none { text(weight: "semibold")[#b.label]; h(0.5em) }
        #b.text
      ]
      v(0.2em)
    } else if b.kind == "application" {
      v(0.3em)
      block(width: 100%, stroke: (left: 2.5pt + luma(80)), inset: 10pt)[
        #set text(size: 10.5pt)
        #if b.audience != none {
          text(weight: "semibold")[Application — #(b.audience)]
          h(0.5em)
        }
        #b.text
      ]
      v(0.2em)
    } else if b.kind == "unknown" {
      v(0.3em)
      text(size: 9pt, fill: luma(110))[Archived directive "#b.name" (preserved verbatim):]
      raw(b.raw, lang: "txt", block: true)
      v(0.2em)
    } else if b.kind == "notes" {
      v(0.3em)
      block(width: 100%, fill: luma(96%), inset: 10pt, radius: 3pt)[
        #set text(size: 10pt)
        #text(weight: "semibold")[Private study notes]
        #b.text
      ]
      v(0.2em)
    }
  }
}
