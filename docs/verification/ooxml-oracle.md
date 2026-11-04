# PPTX craft-ooxml swap (#122): old-vs-new differential

Ticket: cloudbring/artcraft-algorithms#184 (evidence level L3 per research/VERIFICATION.md 1.1).
Run date: 2026-11-04. Machine: Apple Silicon, macOS, `cargo test --release`.

## Setup

- OLD: `deckcraft-pptx` at `3e178d8f5a59c787a6a53a7ed0f052b3e4a1489f` (Release 0.15.0, pre-swap upstream main), linked as `pptx_old` via `package =` rename.
- NEW: the path crate at `168d5da7` (craft-ooxml first at `b78f88e`, then re-pinned to `7d46713`, see Counts), plus the fix described under "Fix made in this ticket".
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

Two runs. "Before" is `168d5da7` + fragment fix with craft-ooxml `b78f88e` (strict reader). "After" re-pins craft-ooxml and craft-archive to craft-libs `7d46713fad25280dc1a3f28e9f68fefa89aa0805` and opens packages with `Tolerance::lenient()` (`crates/pptx/src/opc.rs`; the package `warnings()` are logged as `pptx: <warning>`, like the old reader's `log::warn!`).

| Outcome | Before | After |
|---|---:|---:|
| Identical | 0 | 0 |
| ExpectedDifference | 789 | 797 |
| OnlyNewFails | 8 | 0 |
| BothFail | 2 | 2 |
| Regression / OnlyOldFails / Panic / Timeout | 0 | 0 |
| Total | 799 | 799 |

After, per corpus:

| Corpus | Files | Identical | ExpectedDifference | BothFail | OnlyNewFails | Regression |
|---|---:|---:|---:|---:|---:|---:|
| office-poi | 85 | 0 | 84 | 1 | 0 | 0 |
| office-openxml-sdk | 109 | 0 | 108 | 1 | 0 | 0 |
| office-libreoffice | 589 | 0 | 589 | 0 | 0 | 0 |
| synthetic | 16 | 0 | 16 | 0 | 0 | 0 |

The two corpus fuzzer files (`.../pptx/fail/ofz35597-1.pptx`, `ofz46160-1.pptx`) now open on NEW with a skipped-entry warning and a model equal to OLD's. Expected differences after: 796 `ooxml-package-layout`, 1 `zip-entry-cap-fails-package`. No stale entries. The `zipguard-rejects-backslash-entry-names` entry is removed: with `normalize_backslash_names` the backslash cases open with the same model as OLD, so the ticket's "backslash entry names rejected" change is reversed, not confirmed.

No file is byte-identical after save, because the new writer orders entries differently in every package (D1). Files showing each layout class (a file can show several): part_order 787, rels_order 581, ct_decl 50, reread_rels_order 45. For every file that opens on both sides, the parsed model and every XML and binary part are equal apart from the layout classes. Both sides fail on 2 corpus files (`office-poi/slideshow/Divino_Revelado.pptx`, `office-openxml-sdk/.../encrypted_pptx.pptx`, "Could not find EOCD"); only the error text prefix differs.

## Differences

**D1 `ooxml-package-layout` (796 files, expected).** craft-ooxml `PackageWriter` writes entries sorted (with `[Content_Types].xml` first, where the old writer put each `.rels` next to its part), sorts `Relationship` elements by Id as a string (`rId10` before `rId2`), and declares a `Default Extension` instead of a per-part `Override` when every part with that extension shares a type (craft-libs b78f88e `crates/ooxml/src/opc.rs` lines 490-505, 567, 591). The judge verified the part set, all part bodies, all relationship tuples and the effective content type of every part are equal. OPC treats parts and relationships as unordered sets. Kept chart parts and `customXml` items re-read in that new order (`reread_rels_order`) with identical contents. Confirms the "part ordering" change in the ticket.

**D2 `zip-entry-cap-fails-package` (`synthetic/oversized_part`, expected).** A 513 MiB entry. The caps match the old reader (512 MiB per part, 2 GiB total, 50 000 entries; deckcraft sets `OpcLimits` to these in `Package::open`), so no limit change is needed. The old reader skipped that one part and opened the rest. `ZipGuard` treats any breach of a size cap as a hard error, also in lenient mode (only corrupt or unreadable entries are skipped), so the package is refused. Registered as a security limit: an archive that declares an over-cap entry is refused rather than partly read. User-visible cost: such a file used to open without the part. No corpus file exceeds the cap.

### Resolved by moving to the tolerant reader (were OnlyNewFails in the first run)

Corpus `ofz35597-1.pptx` (bad CRC), `ofz46160-1.pptx` (inflate errors); synthetic `bad_crc_unused_entry`, `dup_rel_id`, `dup_rel_id_unused_part`, `case_duplicate_part`, `broken_rels_xml_unused_part`, `backslash_entry_name`, `backslash_entry_name_part`. Cause was the strict reader at the old pin; craft-libs `7d46713` supplies the opt-in tolerance. The ticket's "duplicate relationship ids skipped" is confirmed with the tolerant reader (strict `b78f88e` failed the open).

### Confirmed no behaviour change (synthetic)

Identical model and package apart from D1: `pct_target_digit` (`slideLayout%31.xml`), `pct_target_dotdot` (`%2e%2e/`), `pct_target_bad_escape` (`a%zzb.xml`), `pct_target_space_part` (part named `slide Layout1.xml` via `%20`), `backslash_target`, `target_escapes_package`, `target_fragment_only`. The old resolver already percent-decoded, so decoding did not change observable results here.

## Fix made in this ticket (regression found by the corpus)

`office-poi/slideshow/with_japanese.pptx` has internal hyperlink relationships with `Target="#_ftn1"` (a bookmark, no part). The model's hyperlink URL was `ppt/slides` before the swap and `ppt/slides/_` after. Cause: `opc::resolve` in `crates/pptx/src/opc.rs` passes a synthetic source `<dir>/_` to `craft_ooxml::opc::resolve_target`, which answers a fragment-only target with the source itself. Fixed in `resolve` (fragment-only returns the base directory as before) with regression test `xml_opc_tests::resolve_fragment_only` in `crates/pptx/src/lib.rs`: red on `168d5da7` (`left: ("ppt/slides/_", "ppt/slides/_", "")`), green after. Note the old value `ppt/slides` is itself not a useful hyperlink target; this restores parity, it does not fix the underlying handling of bookmark links.

## Caveats

- Differential only: it shows the swap preserves old behaviour, not that old behaviour is right.
- The semantic dump is `Debug` output. Media bytes are hashed, not printed. NaN prints as `NaN` on both sides.
- Timing was not measured; no panics or timeouts occurred (300 s limit per file).
- "Oversized-part order" from the ticket was exercised as an oversized part (`synthetic/oversized_part`); nothing in the corpus exceeds the cap.
