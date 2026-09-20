# Original SuperNinja source import

Source archive: _workspace_delivery_sermon-studio-src.tar.gz
SHA-256: d89dc830b74ef3bbb43d95c03e200800c144a278365dade50183874c4821da6b
Frontend baseline before import: 612cb67857461a8c0023f8e492f5529ff42b7e0f

The archive's crates/core, crates/cli, src-tauri, Cargo workspace and lockfile,
scripts, tools, Makefile, ui, and preview are imported without source edits.
Original README, delivery notes, architecture, user guide, data-source notes,
and ignore rules are preserved in docs/superninja. Existing root Rocket files
and docs/ARCHITECTURE.md remain authoritative for the current frontend.
Generated src-tauri/gen schemas are excluded from Git.

The ui directory is the original desktop UI retained for source provenance and
existing build references. The root Next.js application is the current frontend.
The original Tauri config still points to ui/dist; this import does not claim
that Tauri is integrated with the Rocket frontend. Existing IPC contracts differ.
No bundled databases or Linux application binaries are included in this archive.
Historical test/build claims in original delivery documents are not fresh validation.