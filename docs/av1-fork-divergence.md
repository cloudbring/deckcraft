# AV1 fork migration audit (#48)

> **Last reviewed / updated:** 2026-11-03. Consumer migration complete; shared-decoder
> hardening follow-ups remain open. No local AV1 fork or new playback wrapper is retained.

Compared every file in DeckCraft `crates/av1` at `0c593bc1` with `cloudbring/craft-libs`
`algo/48-craft-av1` at `58f8a71e1f29d1ff6238e8eed5215873249550c9`. The shared crate was
ported verbatim from FilmCraft; its README records provenance. All four shared dependencies
(`craft-av1`, `craft-hevc`, `craft-h264`, `craft-bitstream`) now resolve from the same branch and
commit; `Cargo.lock` records that pin. The `TODO(#48)` in the workspace manifest still calls for
repointing to `storytold/craft-libs` after merge.

## File-by-file classification

Of **21 Rust files**, **3 are byte-identical** and **18 differ**: **2 fix-bearing** and
**16 stale implementation, packaging API or formatting drift**. The twenty-second file,
`Cargo.toml`, is packaging. There are four distinct follow-up groups in the two fix-bearing
files. Counts classify each file once; the fix-bearing files also contain stale refactors.
Paths and historical line numbers below refer to the deleted `crates/av1/` at `0c593bc1`.

| Fork file | Classification and disposition |
| --- | --- |
| `Cargo.toml` | Packaging: replace `deckcraft-av1` with shared `craft-av1`; the default `threads` feature remains enabled. Drop local `deckcraft-video-budget` dependency and layer registration. |
| `src/bits.rs` | **Stale:** shared LEB128 rejects unterminated 8-byte encodings; the fork's helper accepted them. |
| `src/cdef.rs` | **Stale:** shared filter separates available/unavailable tap paths and works on row bands; same CDEF stages. |
| `src/cdf.rs` | **Stale:** whitespace-only import formatting; tokens are identical. |
| `src/decoder.rs` | **Fixes F1–F3; API A1.** Store-wide memory accounting, a per-chunk shown-existing-frame output cap and contiguous tile-group ordering are absent upstream. Shared decoding adds a level-based frame-size cap, checked OBU/tile lengths, full tile-coverage validation, panic-to-error worker handling and lower-copy output. |
| `src/frame.rs` | **Stale:** shared plane reuse and row-band splitting for parallel filters replace fork storage. |
| `src/grain.rs` | **Stale:** shared grain generation reorganizes lookup, grain blocks and output processing; no exclusive public grain feature found. |
| `src/header.rs` | **Fix F4.** The fork caps explicit tile columns/rows while parsing, before growing the starts vectors. Shared headers otherwise reorganize parsing and retain strictly increasing film-grain-point validation. |
| `src/inter.rs` | **Stale:** shared motion-compensation argument structs and specialized paths supersede the fork's long-argument block functions. |
| `src/inter_info.rs` | **Stale:** formatting only. |
| `src/intra.rs` | **Stale:** shared prediction takes `IntraParams` instead of repeating its fields; same prediction modes. |
| `src/lib.rs` | **Stale API:** fork re-exports `FrameHeader` and makes `header` public; shared exports the `Decoder`, `Picture`, `ColorConfig` and `SequenceHeader` needed by DeckCraft. No consumer references the removed exports. |
| `src/mvpred.rs` | **Stale:** formatting and motion-vector helper organization. |
| `src/par.rs` | **Identical.** |
| `src/postfilter.rs` | **Stale:** shared loop-filter row bands/iteration supersede the fork's whole-plane path. |
| `src/restoration.rs` | **Stale:** shared restoration separates units and block filtering; same restoration modes. |
| `src/spec_tables.rs` | **Stale:** table layout/formatting drift; the numeric token sequence is identical. |
| `src/state.rs` | **Identical.** |
| `src/stats.rs` | **Stale:** formatting only; tokens are identical. |
| `src/symbol.rs` | **Identical.** |
| `src/tile.rs` | **Stale:** shared decoder splits tile prediction, coefficients, palette, transform sizes and loop restoration into `src/tile/{predict,coeffs,palette,tx_size,loop_restoration}.rs`. The fork's monolithic tile logic needs no local wrapper. |
| `src/transform.rs` | **Stale:** shared const-size inverse transform, lane paths and DC-only optimization replace the scalar stages. |

Shared-only `README.md`, `examples/`, `test-support/`, `tests/`, `tools/` and the five tile
submodules are additional provenance, coverage and decomposition, not deleted fork files.
No DeckCraft-specific playback feature needs a wrapper: `crates/media/src/video.rs` only needs
the existing shared `Decoder`/`Picture` API.

## Shared follow-ups

These are **not fixed by the dependency change**; the deleted fork remains inspectable at
`0c593bc1`. Valid fixture equality does not establish hostile-input parity.

| ID | DeckCraft-only behavior and upstream action |
| --- | --- |
| F1 | `src/decoder.rs:223-241,529-533`: `store_fits` checks padded reference slots, current and in-flight frames against the configured memory budget *before* allocating. The shared level-based dimension/area cap is different; port the total-store limit with checked arithmetic and regressions from `src/decoder.rs:1070-1098`. |
| F2 | `src/decoder.rs:445-450`: reject more than 32 shown-existing-frame outputs in one input chunk. Shared `parse_obus` appends without this bound; port a count bound with a hostile repeated-OBU regression. |
| F3 | `src/decoder.rs:581,982-1003`: reject tile-group gaps, overlap and out-of-order starts. Shared code rejects duplicate/missing tile coverage, but accepts out-of-order groups that eventually cover all tiles; port ordering if required by AV1 §6.10.1 and preserve the fork regression. |
| F4 | `src/header.rs:947-953,967-973`: bound explicit tile columns and rows before allocating their start arrays. The shared uniform path bounds log2 counts but its explicit path lacks these guards; port both checks and adversarial header tests. |

**API A1:** `src/decoder.rs:236-240` exposes `set_memory_budget(u64)`; no DeckCraft consumer
calls it. If the shared crate accepts F1, decide whether to expose this configurability or retain
a fixed default. Shared LEB128, tile-length, frame-size and film-grain-point checks are already
stronger/equivalent; do not reintroduce the fork's older implementations or disable shared tests.

## Fixture evidence and validation

The five existing media video fixtures were **not regenerated or modified**. A temporary
consumer test captured the bytes of all decoded RGBA frames, reverse-seek RGBA frames and
poster PNGs before/after the repoint, then compared each file directly. The permanent
`video_fixture_pixels_match_pre_migration` test additionally checks the original frame/poster
fingerprints and all reverse seeks.

| Fixture | Frames | Reverse seeks | Poster PNGs | Pre/post result |
| --- | ---: | ---: | ---: | --- |
| `clip-h264.mp4` | 5 | 5 | 5 | 15/15 byte-identical |
| `clip-h264.mov` | 6 | 6 | 6 | 18/18 byte-identical |
| `clip-hevc.mp4` | 6 | 6 | 6 | 18/18 byte-identical |
| `clip-vp9.webm` | 5 | 5 | 5 | 15/15 byte-identical |
| `clip-av1.mkv` | 6 | 6 | 6 | 18/18 byte-identical |

Direct `diff -rq` of **84/84 files found zero differences**. The fixture clips themselves were
not changed. This corpus cannot cover all profiles, high resolutions or hostile AV1 inputs.

Validation used the rustup stable compiler and rustdoc with
`CARGO_TARGET_DIR=/Users/emm/ArtCraft/worktrees/deckcraft-48-av1/target-48`:

- `cargo test -p deckcraft-media -p deckcraft-vp9 -p deckcraft-video-budget -p xtask`:
  **94 passed, 0 failed** (media 23 unit + 22 integration, VP9 16 unit + 1 doctest,
  video-budget 3 unit, xtask 29 unit). The fixture fingerprint regression passed.
- `cargo clippy -p deckcraft-media -p deckcraft-vp9 -p deckcraft-video-budget -p xtask
  --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo xtask layers`: passed (25 crates, no violations).
- `cargo xtask wasm`: passed (20 WASM-safe crates). Existing UI `opening_generation`
  and unused egui patch warnings are outside this migration.

Workspace-wide tests/CI, asset and PDF dependency gates were intentionally skipped.
`craft-av1` cannot be directly tested as a dependency from the DeckCraft workspace because
Cargo requires its dev-dependencies and it is not a workspace member; its upstream branch
reported 34/0. No push is performed. The fork deletion removes **20,485 lines across
22 files** (including its manifest).

## Revision history

| Date | Change |
| --- | --- |
| 2026-11-03 | Audited every fork file, repointed four shared crates, removed the fork and checked pre/post fixture bytes. |
