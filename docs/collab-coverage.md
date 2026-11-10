# Live collaboration coverage (CORE-COLLAB-001)

Audit of what a change made by one participant does on the others' screens, for the
features added after Phase 6. How the shared state is laid out:

* **BlueDoc** (`crates/bo-engine/src/collab/mod.rs`, `parts.rs`): the body is the `body`
  Y.Text (one line per block; inline objects — pictures, SmartArt frames, embedded
  objects, fields, content controls, form fields, bookmarks — are one-position embeds
  `{"c": JSON}`), top-level tables cell by cell, stories (notes, comments, headers /
  footers) as nested texts, definitions and settings in `meta` (last writer wins per key),
  binary parts in `blobs`.
* **BlueSheet** (`crates/bo-engine/src/sheet/collab.rs`): cells in `cells` by stable
  row/column ids, formats / row & column properties / merges / rules / charts / pivots in
  `meta`, the rest of each sheet (`sh:<sheet>`: views, page setup, notes, threaded
  comments, form controls, sparklines, slicers, timelines, SmartArt, scenarios …) last
  writer wins per sheet, workbook settings one key each, binary parts in `blobs`.
* **BlueSlide** (`crates/bo-engine/src/slide_edit/collab.rs`): one `slides` entry per
  slide, one `shapes` entry per top-level object, notes text as a nested text, masters /
  layouts / themes / presentation parts in `meta`, package parts in `blobs`.
* **Presence** (awareness, `bo_collab::Presence`): `selection` (BlueDoc caret, UTF-16),
  `cell` (`"<sheet>!<range>"`), `shape` (`"<slide>/<object>"`, or `"<slide>/"` on a slide
  with nothing selected).

"Fixed" = not shared before this item, shared now. Tests: `crates/bo-engine/tests/` unless a
path is given; E2E = `tests/e2e/specs/`.

## BlueDoc

| Feature | Synced? | How | Test |
| --- | --- | --- | --- |
| Text, run / paragraph formatting, lists, sections | yes | `body` Y.Text, attributes | `collab.rs` (`concurrent_typing_*`, `formatting_and_typing_converge`, proptest) |
| Tables (cells, rows, columns, nested, tracked grid) | yes | cell stories + `tb:/tr:/tl:/tc:` | `collab.rs` (`table_*`, `edits_in_different_cells_of_a_nested_table_merge`) |
| Styles, numbering, foot/endnotes, headers/footers, continuation notice | yes | `st:/nm:/an:`, stories, `doc` | `collab.rs` (`styles_comments_notes_and_headers_converge`, …) |
| Comments with @mentions and tasks | yes | `cm:<id>` (author, thread, done, task) + `cm:<id>` story (mention text) | `collab.rs` (`styles_comments_notes_and_headers_converge`, `concurrent_comments_and_notes_get_distinct_ids`) |
| Pictures (insert, crop, compress, replace) | yes | embed + `blobs` | `collab_objects.rs` `doc_pictures_and_smartart_added_at_once_keep_their_own_parts` |
| Pictures / SmartArt added **at once** by two users | **fixed** | new part names are numbered in a band of the replica (`Session::part_number_start`), so both parts survive (before: same `image<n>` name, one replaced the other) | `doc_pictures_and_smartart_added_at_once_keep_their_own_parts` |
| SmartArt (insert, text pane, layout, colours) | **fixed** (part removal) | frame = embed, parts in `blobs`; parts dropped by a rebuild are now removed on the other side too (`blobs` `null`, only when no longer shown) | `doc_pictures_and_smartart_added_at_once_keep_their_own_parts` |
| Embedded / linked workbooks (OLE) — insert | yes | embed + `blobs` | `collab_objects.rs` `an_embedded_workbook_edited_in_place_reaches_the_other_peer` |
| Embedded workbook **edited** (package keeps its part name) | **fixed** | media fingerprints: a part rewritten in place is shared again (before: the peer kept the old workbook, only the preview changed) | `an_embedded_workbook_edited_in_place_reaches_the_other_peer` |
| Content controls, legacy form fields, signature lines, equations, text boxes | yes | inline embeds (`ControlStart`, `FormField`, `Image`, …) | `collab.rs` proptest / `joiner_typing_anywhere_in_a_real_file_converges` (generic embed path) |
| Custom XML parts (add / replace / remove / values) | **fixed** | `meta` `cx` (§15.2.4 / §15.2.5) | `collab_objects.rs` `doc_settings_set_at_once_by_two_peers_converge`, `doc_settings_reach_a_late_joiner` |
| Content control data binding | yes (binding) + **fixed** (data) | binding travels with the control embed, bound data in `cx` | same |
| Page colour, theme, theme fonts, style set (Design tab) | **fixed** | `meta` `ds` → `Op::Design` | `doc_settings_set_at_once_by_two_peers_converge` |
| Watermark, page borders | yes | section properties (`sect`) / header story + `blobs` | `collab.rs` `header_text_is_shared` (story path) |
| Hyphenation settings | **fixed** | `meta` `hy` → `Op::Hyphenation` | `doc_settings_set_at_once_by_two_peers_converge` |
| Restrict editing (`documentProtection`) and editable sections | **fixed** (protection) | `meta` `pt` → `Session::set_protection`; unprotected sections travel in `sect` | `doc_settings_set_at_once_by_two_peers_converge`, `doc_settings_reach_a_late_joiner` |
| Track changes: revisions | yes | revision marks are run attributes / embeds | `collab.rs` `tracked_grid_change_is_shared` |
| Track changes: on/off switch | per user (by design) | session mode, like the reviewer's own choice; forced on for everyone when `pt` locks tracking | — |
| Mail merge: data source, recipients, preview | per user (by design) | engine state outside the document (`merge_cmd.rs`); the file's `w:mailMerge` settings are kept verbatim | — |
| Mark as final, sensitivity label, passwords (`security_cmd.rs`) | not shared | applied when saving (file-level security, `Security` state per editor) | — (listed as remaining work) |
| PDF export choices | per user (by design) | options of one export, not document state | — |
| Version restore | yes | the server resets the room (`CollabRoom.ResetAsync`, close 4001 `reload`); every participant reloads the restored version | `packages/collab/test/client.test.ts` (reload close), `apps/server/BlueOffice.Tests/FileApiTests.cs` |
| **Presence**: caret + name, selection | yes | awareness `selection` → `collabPeers` | E2E `collab-doc.spec.ts`, `packages/canvas-host/test/peers.test.ts` |
| **Presence**: bar in the margin of the paragraph being edited + name | **new** | `PeerRects.para` (`nav::paragraph_band`) → `DocumentView` `peer-bar` | `collab_objects.rs` `doc_presence_has_the_paragraph_band`, `peers.test.ts`, E2E `collab-doc.spec.ts` |

## BlueSheet

| Feature | Synced? | How | Test |
| --- | --- | --- | --- |
| Values, formulas (refs by row/column id), formats, row/column insert & delete, sizes, merges, sheets | yes | `cells`, `fm:`, `ra:/rd:/ca:/cd:`, `rp:/cp:`, `mg:`, `sh:` | `collab_sheet.rs` |
| Conditional formats, data validation, charts, pivots, defined names, AutoFilter, tables, print area | yes | `cf:/dv:/ch:/pv:/pc:/wb/sha:` | `collab_sheet.rs` (`*_during_a_row_insert_follows_the_rows`) |
| Sparklines, form controls, slicers, timelines, scenarios, page breaks | yes | `sh:<sheet>` (+ placement kept on row/column changes) | `collab_sheet.rs` `controls_and_sparklines_shift_once_with_a_row_insert_from_the_same_peer` |
| SmartArt in a sheet | yes | `SheetDiagram` (rebuilt XML) in `sh:<sheet>` | generic `sh:` path (`collab_sheet.rs`) |
| Notes and threaded comments with @mentions / tasks | yes | `sh:<sheet>` | `collab_objects.rs` `sheet_theme_persons_and_workbook_protection_converge` |
| Threaded-comment **persons** (authors, mentioned people) | **fixed** | `meta` `pe:<id>`, one per person (before: a new person's id had no name on the other side) | `sheet_theme_persons_and_workbook_protection_converge` |
| Checkbox cells | yes | carried by the cell format (`CellXf.checkbox`) in `fm:` | `collab_objects.rs` `sheet_cell_pictures_and_checkboxes_converge` |
| Pictures in cells (`_localImage`) | **fixed** | cell entry `im` + bytes once in `blobs` (`cellimg:<hash>`) | `sheet_cell_pictures_and_checkboxes_converge` |
| `IMAGE()` pictures | yes | the formula is shared; each replica fetches the picture | `collab_sheet.rs` (formula path) |
| Theme applied from a file (`.thmx`) | **fixed** | theme colours `wt`, the theme part in `blobs`, workbook rels `wr` | `sheet_theme_persons_and_workbook_protection_converge` |
| Workbook structure protection | **fixed** | `meta` `wk` (§18.2.29, verbatim) | `sheet_theme_persons_and_workbook_protection_converge` |
| Sheet protection, allowed ranges | yes | `sh:<sheet>` (`unknown`: `sheetProtection`, `protectedRanges`) | generic `sh:` path |
| `date1904` | **fixed** | `meta` `wd` | (same entry code as `wk`) |
| Embedded workbook as a source (doc/slide objects) | n/a | edited inside the host document (BlueDoc / BlueSlide rows) | — |
| Custom views, sort / filter state, freeze panes | yes | `sh:<sheet>` / `sha:<sheet>` | `collab_sheet.rs` `panes_frozen_during_a_row_insert_follow_the_rows` |
| **Presence**: coloured border on the selected cell / range + name tag | **new** | awareness `cell` via `SheetHost.collabSelect` / `onSheetPeers` (`packages/collab/src/sheet.ts`), drawn by `packages/sheet-editor/src/peers.tsx` | `packages/sheet-editor/test/peers.test.tsx`, `packages/collab/test/presence.test.ts`, E2E `collab-sheet-slide.spec.ts` |

## BlueSlide

| Feature | Synced? | How | Test |
| --- | --- | --- | --- |
| Slides (add, delete, move, properties, transitions, timing), objects, text | yes | `slides` / `shapes` maps, order keys | `collab.rs` (`slide_move_and_text_edit_converge`, `concurrent_slide_moves_keep_both_moves`) |
| Notes | yes | nested text `n:<slide>` | `collab.rs` `notes_text_merges_and_parts_follow_the_room` |
| Masters, layouts, themes, sections, custom shows | yes | `sm:/sl:/th:`, `pp` (presentation extensions) | `collab.rs` `designs_imported_at_once_by_two_peers_are_both_kept` |
| Slide size | **fixed** | `meta` `ps` (§19.2.1.39 sldSz, §19.2.1.23 notesSz) | `collab_objects.rs` `slide_size_change_reaches_the_other_peer` |
| Pictures, media (video / audio) inserted | yes | object entry + new parts in `blobs` | `collab.rs` `pictures_pasted_at_once_by_two_peers_keep_their_own_parts` |
| Media trim, fade, bookmarks, playback options | yes | `p14:media` extension of the object (shape entry) + timing in the slide entry | generic shape path (`collab.rs` `shapes_round_trip_json`) |
| Comments with @mentions / tasks | **fixed** | comment and author parts are rewritten in place: part fingerprints re-share them (before: only the first comment of a slide arrived) | `collab_objects.rs` `slide_comment_parts_rewritten_in_place_converge` |
| SmartArt (insert, text pane, layout, colours) | **fixed** (edits) | frame = object; diagram parts rewritten in place are shared again | same mechanism as comments (`slide_comment_parts_rewritten_in_place_converge`) |
| Ink: new strokes | yes | one ink object + InkML part each | generic part path |
| Ink: erased traces of ink read from a file | **fixed** | the InkML part is rewritten in place → re-shared by fingerprint | same mechanism |
| Embedded workbooks (insert / edit in place) | yes / **fixed** | object + parts; edited package re-shared by fingerprint | same mechanism (doc: `an_embedded_workbook_edited_in_place_reaches_the_other_peer`) |
| Charts | yes | chart parts rewritten → re-shared; series data in the object | same mechanism |
| Slide show settings, rehearsed timings, recordings | yes | `pp` / slide timing / new parts | generic paths |
| Version restore | yes | room reset (see BlueDoc) | — |
| **Presence**: outline on the selected object + name | yes | awareness `shape` → `slideCollabPeers` | E2E `collab-sheet-slide.spec.ts` |
| **Presence**: thumbnail badge with initials on the slide each user is on | **new** | `SlideCollabOut.slide` → awareness `"<slide>/"` when nothing is selected; `PeerBadges` on the thumbnails | `collab_objects.rs` `slide_presence_names_the_current_slide`, `packages/slide-editor/test/peerBadges.test.tsx`, `packages/collab/test/presence.test.ts`, E2E `collab-sheet-slide.spec.ts` |

## Known limits

* Last writer wins per key for whole parts and settings: two users changing the same
  setting (or adding comments to the same slide, which share one comment part) at the
  same instant keep the later write; different settings / slides merge.
* File-level security of BlueDoc (mark as final, sensitivity label, open/modify
  passwords) is applied per editor when saving and is not shared yet.
* Presence of a BlueSheet selection is an A1 range: a row inserted by someone else moves
  the frame only when its owner selects again.
