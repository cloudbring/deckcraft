# HEVC fork migration audit (#48)

> **Last reviewed / updated:** 2026-11-03. Consumer migration complete; five shared-decoder
> hardening follow-ups remain open below. No codec fork or new decoding wrapper is retained.

Compared every file under DeckCraft `crates/hevc` at
`a42c666684ca8b96d4d7835848b7822d589d98e5` against `cloudbring/craft-libs`
branch `algo/48-craft-hevc` at `00fbb685e7b10e37717adee3c7285705b2b3de9f`.
The shared crate comes from FilmCraft `eb3487a5`; its provenance is in its README.
The prepared `algo/48-use-craft-hevc` worktree was still at `a42c6666`, not a later
`algo/48-h264-hardened` consumer commit. This change builds on that existing base and
repoints **all three** shared dependencies (`craft-hevc`, `craft-h264`, `craft-bitstream`)
to the HEVC branch, which already contains the hardened H.264 implementation.
The lockfile records the exact shared commit above; unrelated pins are unchanged.

## File-by-file classification

Of **17 Rust files**, **4 are byte-identical** and **13 differ**: **4 carry fixes to
upstream**, **1 carries regression coverage to port**, and **8 are stale implementation,
API-internal or formatting drift**. The eighteenth file, `Cargo.toml`, is packaging.
Counts classify each file once; a fix-bearing file can also contain stale differences.
There are **five distinct hardening groups** across the four fix-bearing files, and
**one unused configuration API** to list for upstreaming, not a new playback feature.

Paths in this table are relative to the deleted `crates/hevc/`. Historical line numbers
refer to `a42c6666` and remain retrievable with `git show a42c6666:crates/hevc/src/<file>`.

| Fork file | Classification and disposition |
| --- | --- |
| `Cargo.toml` | Packaging: replace the workspace crate with `craft-hevc`; retain the shared default `threads` feature. Remove its workspace dependency and layer registration. |
| `src/cabac.rs` | **Identical**; use shared CABAC. |
| `src/error.rs` | **Identical**; use shared errors. |
| `src/spec_tables.rs` | **Identical**; retain shared specification-table provenance. |
| `src/tables.rs` | **Identical**; use shared scans and tables. |
| `src/decoder.rs` | **Fixes F1/F2; API A1.** Typed two-pass `hvcC` preflight and whole-picture-store budget are absent upstream. Remaining frame-job, output-plane and block-count refactors are stale. The fork's capped missing-reference allocation is superseded by shared gray sample backing with distinct picture identities. |
| `src/dpb.rs` | **Stale:** fixed `FrameRef` DPB becomes generic `Dpb<F>`, `Ref<F>`, `RefPicSet<F>` and `Output<F>` upstream; DPB/RPS algorithms are otherwise equivalent apart from formatting. |
| `src/filter.rs` | **Stale:** shared generic sample buffers, batched deblocking, SAO decomposition and checked slice-reference lookup supersede the old `u16` implementation. |
| `src/inter.rs` | **Stale:** shared direct reference-row access, generic samples, narrow-lane interpolation and weighted-prediction specializations supersede the window-copy path. No exclusive prediction mode. |
| `src/intra.rs` | **Stale:** shared generic/const-size planar, DC and angular prediction (including horizontal transpose) replaces the dynamic `u16` loops. Modes and filtering remain supported. |
| `src/lib.rs` | **Stale:** crate-name example, lint allowances and test adapter; upstream additionally exports generic `dpb`. Public `Decoder`, `Picture`, `Plane` and color metadata still fit the existing media adapter. |
| `src/mvpred.rs` | **Stale:** `PbGeom` plus separate partition argument becomes `PredBlock`; merge/AMVP helper extraction and generic decoder plumbing differ, not motion-vector semantics. |
| `src/params.rs` | **Fix F3.** Pre-addition PPS QP range validation is absent upstream. Other checked SPS/RPS parsing is already present or equivalent; upstream adds an aggregate luma cap and saturating crop subtraction. The fork's distinct hostile-input tests are listed below. |
| `src/picture.rs` | **Stale:** shared `u8`/`u16` row storage, cached gray backing, safe missing-row reads, direct runs and cropped-plane assembly supersede the old all-`u16` storage. |
| `src/slice.rs` | **Fixes F4/F5.** Checked slice-QP sum and pre-addition chroma-weight-denominator validation are absent upstream. Parser method/free-function decomposition is stale. Shared entry-point parsing already checks `+1` overflow and has regressions. |
| `src/slicedec.rs` | **Fix F4 (second site).** Checked slice-QP sum at decode initialization is absent upstream. Rest is stale: generic sample state, z-order availability masks, WPP/dependent-context decomposition, and coding/prediction/transform work split into three shared submodules. |
| `src/synth_tests.rs` | **Regression coverage:** port the budget and typed-configuration tests with F1/F2. Shared synthetic tests restore the FFmpeg oracle that the fork disabled and add missing-reference cases; preserve that stronger shared coverage. |

Upstream-only files are additions, not DeckCraft-only features: `README.md`, `rustfmt.toml`,
`tools/extract_tables.py`, `examples/hevcdec.rs`, `test-support/mod.rs`,
`tests/fuzz_hostile.rs`, `tests/it/{main,conformance,robustness,perf}.rs`,
`tests/it/common/mod.rs`, and `src/slicedec/{coding_unit,prediction_unit,transform_tree}.rs`.
The moved CU, PU and transform logic was compared with the corresponding sections of the
fork's monolithic `slicedec.rs`, not treated as missing functionality.

## Precise upstream work list

These are **not fixed by changing the dependency**. Valid fixture equality does not establish
hostile-input safety. They belong in `craft-hevc` before #48 is considered fully closed.
Shared line numbers below refer to `00fbb68`, under the same `crates/hevc/src/` path.

- **F1 — configuration validation:** fork `decoder.rs:160` preflights every nonempty
  `hvcC` array's structure and NAL header before calling `handle_nal`: only VPS/SPS/PPS
  and prefix/suffix SEI are allowed, and each NAL type must match its array declaration
  (`decoder.rs:175`). It sets `nal_length_size` only after both passes (`decoder.rs:191`).
  Shared `decoder.rs:160` ignores the declared array type, handles NALs immediately and
  changes framing before checking the arrays. Port the structural/type preflight and
  delayed framing update; preserve accepted SEI. This is not full transactional rollback:
  the old second pass can still mutate parameter stores before a later semantic parse error.
  Add mismatched-type, forbidden VCL/AUD, truncated-late-array and unchanged-framing tests.
- **F2 — whole-picture-store budget:** fork `decoder.rs:397` checks sample bytes for
  `max_dec_pic_buffering + 1 + max_in_flight` pictures before allocation at
  `decoder.rs:418`. Its shape uses chroma subsampling, two bytes/sample, and checked
  `deckcraft_video_budget::PictureShape` arithmetic. Defaults are 4 GiB on 64-bit and
  1 GiB on 32-bit. Shared `decoder.rs:382` lacks this check; its `params.rs:11`
  `MAX_LUMA_SAMPLES = 1 << 26` cap limits one picture, not the store plus workers.
  Port the aggregate limit into the shared crate, adapting accounting to its `u8`/`u16`
  storage and shared gray rows; do not add a dependency back to DeckCraft. The fork's
  sample-byte estimate is not a complete allocator/work-buffer accounting scheme.
- **F3 — PPS initial QP overflow:** fork `params.rs:731` validates
  `init_qp_minus26` in `-74..=25` before adding 26. Shared `params.rs:727` directly
  computes `26 + read_se()?`, which can overflow for `i32::MAX` before validation.
  Port the broad pre-SPS range check, including boundary and extreme-value tests.
- **F4 — slice QP overflow, two sites:** fork `slice.rs:304` and `slicedec.rs:330`
  use `checked_add` for `pps.init_qp + slice_qp_delta` and return `Error::Invalid`.
  Shared `slice.rs:394` and `slicedec.rs:377` use unchecked addition. Port both
  checks; retain the SPS-bit-depth-dependent range validation. Exercise extreme signed
  deltas with a valid PPS, rather than relying only on F3.
- **F5 — weighted-prediction denominator overflow:** fork `slice.rs:438` validates
  `delta_chroma_log2_weight_denom` in `-7..=7` before adding the luma denominator.
  Shared `slice.rs:453` adds the hostile signed Exp-Golomb value first. Port the
  pre-addition check and test a positive luma denominator plus `i32::MAX`, as well as
  valid boundary sums and out-of-range negative deltas.

**A1 — configuration API:** `decoder.rs:392`, `set_memory_budget(bytes: u64)`, is the
only lost public decoder option. No DeckCraft production caller uses it. List it for
shared implementation alongside F2; do not create a misleading media wrapper that can
only inspect already-allocated pictures. The existing `Hevc` adapter still owns RGBA
conversion and reset/seek behavior. `deckcraft-video-budget` remains for AV1 and VP9;
its mere presence does not protect `craft-hevc`.

Port regression coverage from fork `decoder.rs:675` (SEI arrays), `params.rs:1042`
(QP bounds), `synth_tests.rs:635` (exact whole-store budget and one-byte-under),
`synth_tests.rs:649` (valid VPS/SPS/PPS configuration) and `synth_tests.rs:664`
(valid VCL rejected without starting a picture). Fork `params.rs:983` and
`params.rs:994` cover hostile SPS/crop and RPS deltas: retain equivalent coverage
upstream, but these are not additional missing arithmetic fixes.

## Fixture evidence

Before repointing, a temporary integration harness captured every decoded RGBA frame,
every reverse seek at `frame_index * 0.1 + 0.05` seconds, and a poster PNG at each of
those positions. The same harness ran after repointing. `diff -rq` compared actual
output bytes, not just hashes: **84/84 outputs identical, zero differences**.
The permanent `video_fixture_pixels_match_pre_migration` integration regression
records the original FNV-1a 64-bit frame/poster fingerprints and checks all reverse
seeks, plus the existing initial-poster check. No fixture was regenerated or modified.

| Fixture | RGBA frames | Reverse seeks | Poster PNGs | Result |
| --- | ---: | ---: | ---: | --- |
| `clip-h264.mp4` | 5 | 5 | 5 | Byte-identical, including hardened H.264 |
| `clip-h264.mov` | 6 | 6 | 6 | Byte-identical, including hardened H.264 |
| `clip-hevc.mp4` | 6 | 6 | 6 | Byte-identical |
| `clip-vp9.webm` | 5 | 5 | 5 | Byte-identical with the repinned bitstream |
| `clip-av1.mkv` | 6 | 6 | 6 | Byte-identical |

`clip-vp8.webm` has no decoded pixels to compare: the unchanged media dispatch rejects
VP8 as unsupported. A regression test now locks in `Unsupported("VP8 video")`.
The existing damaged/truncated-media regressions also remain in the media suite.
This small corpus does not cover every HEVC profile, tool or hostile-input case.

## Validation and scope

All build artifacts, temporary source comparisons and byte captures use
`CARGO_TARGET_DIR=/Users/emm/ArtCraft/worktrees/deckcraft-48-hevc/target-48`, removed
after validation. Commands use the rustup stable toolchain with its `bin` directory
first in `PATH` and explicit rustup `RUSTC`/`RUSTDOC`, including nested xtask commands.
The first WASM attempt selected Homebrew's compiler through `PATH` and failed to find
WASM `core`/`std`; rerunning with the rustup paths fixed that environment-only failure.

Final validation with the rustup stable toolchain:

- `cargo test -p deckcraft-media -p deckcraft-vp9 -p deckcraft-video-budget -p xtask`:
  **94 passed, 0 failed** (media: 23 unit + 22 integration; VP9: 16 unit + 1 doctest;
  video-budget: 3 unit; xtask: 29 unit).
- `cargo clippy -p deckcraft-media -p deckcraft-vp9 -p deckcraft-video-budget -p xtask --all-targets -- -D warnings`:
  passed.
- `cargo fmt --all -- --check`: passed.
- `cargo xtask layers`: passed, 26 crates, no violations.
- `cargo xtask wasm`: passed, all 21 WASM-safe crates. Existing missing sibling
  craft-fonts and UI `opening_generation` warnings remain outside this migration.

Workspace-wide CI, tests, clippy, assets and PDF dependency gates are intentionally
skipped as requested. The shared crate's reported 89/0 source tests were not rerun
locally; the results above are the consumer validation. Upstream hardening F1–F5
and the unused budget API A1 remain separate work.
Deleting the fork removes **7,384 lines in 18 files** (7,364 Rust lines plus the manifest).
No push is performed.

## Revision history

| Date | Change |
| --- | --- |
| 2026-11-03 | File-by-file audit, shared dependency migration, and pre/post fixture comparison. |
