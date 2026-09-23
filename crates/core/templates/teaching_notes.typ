// Sermon Studio — Bible study / teaching notes template.
// Template id: teaching_notes — version 1.0.0 (keep in sync with export.rs).
//
// Teacher-facing handout compiled from the same canonical `sermon` dict as
// the pulpit and bulletin templates: primary passage, Big Idea (optional),
// movements, illustrations, applications, private exegetical notes (only when
// explicitly included upstream), and — when requested — a For Discussion
// section from `:::discussion` blocks.
//
// Deterministic by construction: never queries the current date, document
// date disabled.

// Optional-field fallback (Typst `or` is boolean-only).
#let o(x, alt) = if x != none { x } else { alt }

#let teaching-doc(sermon) = {
  set document(
    title: sermon.meta.title,
    author: "Sermon Studio",
    date: none,
  )
  set page(
    paper: "a4",
    margin: (x: 2cm, y: 2.2cm),
  )
  set text(font: "Libertinus Serif", size: 11pt, lang: "en")
  let spacing = if sermon.teaching != none { sermon.teaching.spacing } else { "comfortable" }
  set par(
    justify: false,
    leading: if spacing == "compact" { 0.58em } else { 0.72em },
    spacing: if spacing == "compact" { 0.8em } else { 1.0em },
  )
  set heading(numbering: none)

  let teacher-headings = if sermon.teaching != none { sermon.teaching.teacher_headings } else { true }
  let include-big-idea = if sermon.teaching != none { sermon.teaching.include_big_idea } else { true }
  let include-discussion = if sermon.teaching != none { sermon.teaching.include_discussion } else { false }

  // ---- Header ------------------------------------------------------------
  [
    #set text(size: 17pt, weight: "bold")
    #(sermon.meta.title)

    #v(0.2em)
    #set text(size: 10pt, fill: luma(90))
    #o(sermon.meta.passage, "")
    #if sermon.meta.date != none [ · Preached #(sermon.meta.date)]
    #if sermon.meta.series != none [ · #(sermon.meta.series)]

    #if include-big-idea and sermon.meta.big_idea != none [
      #v(0.4em)
      #set text(size: 11.5pt, style: "italic")
      #if teacher-headings [Big Idea — ]
      #(sermon.meta.big_idea)
    ]

    #v(0.4em)
    #line(length: 100%, stroke: 0.7pt + luma(60))
    #v(0.2em)
  ]

  // ---- Body --------------------------------------------------------------
  let discussion-blocks = ()

  for b in sermon.blocks {
    if b.kind == "markdown" {
      b.text
    } else if b.kind == "movement" {
      let title-part = if b.title != none [ — #b.title] else []
      let head = if b.order != none [
        #if teacher-headings [Movement ]#b.order#title-part
      ] else [
        #if teacher-headings [Movement]#title-part
      ]
      heading(level: 2, head)
      if b.warrant != none {
        text(size: 10pt, fill: luma(110))[#b.warrant]
        v(0.15em)
      }
      if b.manuscript != none { b.manuscript }
      for item in b.bullets [- #item]
    } else if b.kind == "illustration" {
      v(0.2em)
      if b.label != none {
        text(size: 10.5pt)[#text(weight: "semibold")[#b.label] — #b.text]
      } else {
        text(size: 10.5pt)[#b.text]
      }
      v(0.1em)
    } else if b.kind == "application" {
      v(0.2em)
      if teacher-headings {
        heading(level: 2, if b.audience != none [Application — #(b.audience)] else [Application])
      }
      if b.audience != none and not teacher-headings {
        text(size: 10.5pt)[#text(weight: "semibold")[Application — #(b.audience)] #b.text]
      } else {
        b.text
      }
      v(0.1em)
    } else if b.kind == "notes" {
      // Present only when private study notes were explicitly included.
      v(0.2em)
      heading(level: 2, if teacher-headings [Exegetical Notes] else [Notes])
      set text(size: 10.5pt)
      b.text
      v(0.1em)
    } else if b.kind == "discussion" {
      discussion-blocks.push(b.text)
    } else if b.kind == "unknown" {
      // Archival preservation of unknown directives, as in every format.
      v(0.15em)
      text(size: 9pt, fill: luma(110))[#b.raw]
      v(0.1em)
    }
  }

  // ---- For Discussion / Q&A (only when requested upstream) ---------------
  if include-discussion and discussion-blocks.len() > 0 {
    heading(level: 2, if teacher-headings [For Discussion] else [Discussion])
    for d in discussion-blocks {
      d
    }
  }

  v(0.6em)
  line(length: 100%, stroke: 0.4pt + luma(190))
  text(size: 8pt, fill: luma(130))[Exported from Sermon Studio.]
}
