# PPTX craft-ooxml swap (#122): old-vs-new differential

Ticket: cloudbring/artcraft-algorithms#184 (evidence level L3 per research/VERIFICATION.md 1.1).
Run date: 2026-11-04. Machine: Apple Silicon, macOS, `cargo test --release`.

## Setup

- OLD: `deckcraft-pptx` at `3e178d8f5a59c787a6a53a7ed0f052b3e4a1489f` (Release 0.15.0, pre-swap upstream main), linked as `pptx_old` via `package =` rename.
- NEW: the path crate at `168d5da7` (craft-ooxml pinned to craft-libs `b78f88e`), plus the one fix described under "Fix made in this ticket".
- Harness: `oracle/tests/ooxml_oracle.rs` (crate `deckcraft-ooxml-oracle`, workspace member, not a default member). craft-oracle and craft-compare at craft-libs `313fbe2da4b63c649e6bdfb502741008f4a97c83`.
- Run:
  `DECKCRAFT_OFFICE_CORPUS=/Users/emm/ArtCraft/corpus cargo test --release -p deckcraft-ooxml-oracle --test ooxml_oracle -- --ignored --nocapture`
  Output goes to `$DECKCRAFT_ORACLE_OUT` (default `target/oracle`): `report.md`, `report.json`, `summary.md`, `details.json` (every difference note per file). `DECKCRAFT_INSPECT=<file>` with the `inspect_one` test writes `old.pptx`/`new.pptx` for unzip and diff.
- Expected differences: `oracle/expected.toml`.
- Corpus: every `.pptx/.pptm/.potx/.ppsx` under `office-poi` (85), `office-openxml-sdk` (109), `office-libreoffice` (589) = 783 files, read in place and not copied. Plus 16 generated packages (`synthetic/*`) for behaviour changes the corpus does not contain; each differs from `synthetic/control` in one thing.

Per file, both sides run `import_checked`, then `export`, then `import_checked` on the saved bytes. Judged on:

1. open ok or the same error (craft-oracle `BothFail` / `OnlyOldFails` / `OnlyNewFails`);
2. the semantic model: `{:?}` of the whole `Presentation` (media bytes replaced by length plus FNV hash) and the list of slides skipped as unreadable;
3. the saved package: same part set; same part order; every `.xml/.rels/.vml` part equal as canonical XML (craft-compare `compare_xml`); every other part byte-equal; `[Content_Types].xml` compared by effective content type per part;
4. the model re-read from the saved package.

The judge collapses differences into classes. `package_layout` = {part_order, rels_order, ct_order, ct_decl, dump_rels_order, reread_rels_order}, only entry order, relationship order, Default-vs-Override style. Anything else keeps its own class name.

## Counts

| Corpus | Files | Identical | ExpectedDifference | OnlyNewFails (open regression) | BothFail | OnlyOldFails | Regression | Panic/Timeout |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| office-poi | 85 | 0 | 84 | 0 | 1 | 0 | 0 | 0 |
| office-openxml-sdk | 109 | 0 | 108 | 0 | 1 | 0 | 0 | 0 |
| office-libreoffice | 589 | 0 | 587 | 2 | 0 | 0 | 0 | 0 |
| synthetic | 16 | 0 | 10 | 6 | 0 | 0 | 0 | 0 |
| Total | 799 | 0 | 789 | 8 | 2 | 0 | 0 | 0 |

No file is byte-identical after save, because the new writer orders entries differently in every package (see D1). Of the 789 expected differences: 787 are `ooxml-package-layout`, 2 are `zipguard-rejects-backslash-entry-names`. Registry has no stale entries.

Files showing each layout class (a file can show several): part_order 778, rels_order 573, ct_decl 50, reread_rels_order 45.

Semantic model: equal for all 783 corpus files that open on both sides, after the fix below. Before the fix, 1 file (`office-poi/slideshow/with_japanese.pptx`) differed. Saved XML parts and binary parts: equal for every file that opens on both sides, apart from the layout classes above.

Both sides fail on 2 corpus files (`office-poi/slideshow/Divino_Revelado.pptx`, `office-openxml-sdk/.../encrypted_pptx.pptx`, both "Could not find EOCD"); only the error text prefix differs (`ZIP error:` added).

## Differences

### Registered as expected

**D1 `ooxml-package-layout` (787 files).** craft-ooxml `PackageWriter` writes entries sorted (with `[Content_Types].xml` first, where the old writer put each `.rels` next to its part), sorts `Relationship` elements by Id as a string (`rId10` before `rId2`), and declares a `Default Extension` instead of a per-part `Override` when every part with that extension shares a type (craft-libs b78f88e `crates/ooxml/src/opc.rs` lines 490-505, 567, 591). The judge verified the part set, all part bodies, all relationship tuples and the effective content type of every part are equal. OPC treats parts and relationships as unordered sets. Kept chart parts and `customXml` items re-read in that new order (`reread_rels_order`), also with identical contents. Confirmed, not reversed: the "part ordering" change in the ticket.

**D2 `zipguard-rejects-backslash-entry-names` (2 synthetic files).** A zip entry name containing `\` makes `ZipGuard` fail the whole package ("unsafe ZIP entry path"). Old code opened it and rewrote `\` to `/`. Intended hardening per the ticket; note the cost: such a file is now unopenable, not degraded. No corpus file has one.

### Open: OnlyNewFails, not registered (8 files)

Root cause for all: the craft-ooxml pinned at `b78f88e` is a strict reader. `OpcPackage::open` returns an error for the whole package where the old deckcraft reader logged a warning and carried on. craft-libs `7056f25`/`4d7f516` (on `algo/integration`, after the pin) add an opt-in tolerant reader that skips bad relationships and warns; deckcraft does not use it yet (its `LIMITS` set `ReadMode::Lenient`, which only covers relationship targets, not the cases below). These need a decision: adopt the tolerant reader, or accept and register each with a justification. None is registered here because no justification exists in the repo.

| File | Old | New error | Strict check |
|---|---|---|---|
| corpus `office-libreoffice/sd/qa/unit/data/pptx/fail/ofz35597-1.pptx` | opens | `ZIP I/O: Invalid checksum` | CRC of every entry (slide1.xml, thumbnail.jpeg bad) |
| corpus `.../fail/ofz46160-1.pptx` | opens | `ZIP I/O: incomplete deflate stream` | any entry fails to inflate (about 17 entries damaged) |
| `synthetic/bad_crc_unused_entry` | opens | `Invalid checksum` | unreferenced entry with bad CRC |
| `synthetic/oversized_part` | opens, skips the part | `ZIP entry exceeds byte limit` | one 513 MiB entry over the 512 MiB cap |
| `synthetic/dup_rel_id` | opens | `duplicate relationship id` | duplicate Id in slide1 rels |
| `synthetic/dup_rel_id_unused_part` | opens | `duplicate relationship id` | duplicate Id in `_rels/.rels` |
| `synthetic/case_duplicate_part` | opens | `case-duplicate part` | `PPT/presProps.xml` beside `ppt/presProps.xml` |
| `synthetic/broken_rels_xml_unused_part` | opens | `invalid XML` | unrelated layout's `.rels` is malformed |

The ticket expected "duplicate relationship ids skipped". At the pinned rev they are not skipped: they fail the open. That behaviour exists only in the later tolerant mode. The two corpus files are fuzzer cases under a `fail/` directory (LibreOffice expects them to be rejected or degraded), so the corpus impact is 2 of 783; the synthetic cases show the general loss of tolerance.

### Confirmed no behaviour change (synthetic)

Identical model and package apart from D1: `pct_target_digit` (`slideLayout%31.xml`), `pct_target_dotdot` (`%2e%2e/`), `pct_target_bad_escape` (`a%zzb.xml`), `pct_target_space_part` (part named `slide Layout1.xml` via `%20`), `backslash_target` (`..\slideLayouts\...`), `target_escapes_package`, `target_fragment_only` (after the fix). So percent-decoding of targets did not change observable results here (the old resolver already decoded; new rejects malformed escapes, which lenient mode turns into a skipped relationship).

## Fix made in this ticket (regression found by the corpus)

`office-poi/slideshow/with_japanese.pptx` has internal hyperlink relationships with `Target="#_ftn1"` (a bookmark, no part). The model's hyperlink URL was `ppt/slides` before the swap and `ppt/slides/_` after. Cause: `opc::resolve` in `crates/pptx/src/opc.rs` passes a synthetic source `<dir>/_` to `craft_ooxml::opc::resolve_target`, which answers a fragment-only target with the source itself. Fixed in `resolve` (fragment-only returns the base directory as before) with regression test `xml_opc_tests::resolve_fragment_only` in `crates/pptx/src/lib.rs`: red on `168d5da7` (`left: ("ppt/slides/_", "ppt/slides/_", "")`), green after. Note the old value `ppt/slides` is itself not a useful hyperlink target; this restores parity, it does not fix the underlying handling of bookmark links.

## Caveats

- Differential only: it shows the swap preserves old behaviour, not that old behaviour is right.
- The semantic dump is `Debug` output. Media bytes are hashed, not printed. NaN prints as `NaN` on both sides.
- Timing was not measured; no panics or timeouts occurred (300 s limit per file).
- "Oversized-part order" from the ticket was exercised as an oversized part (`synthetic/oversized_part`); nothing in the corpus exceeds the cap.
