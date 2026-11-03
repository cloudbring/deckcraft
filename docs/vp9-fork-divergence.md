# VP9 fork migration audit (#48)

> **Last reviewed / updated:** 2026-11-03. Consumer migration complete; shared-decoder
> memory-budget hardening remains a follow-up. No local VP9 fork or new wrapper is retained.

Compared each DeckCraft `crates/vp9` file at `a77db266` with `cloudbring/craft-libs`
`algo/48-craft-vp9` at `9aafc3f89c4ce45dc12d5392b194c759a18cf3b0`. The shared VP9
crate comes from FilmCraft and uses the shared `craft-bitstream` boolean decoder; the same
branch contains `craft-vp8`. All five DeckCraft shared codec dependencies (`craft-bitstream`,
`craft-h264`, `craft-hevc`, `craft-av1`, `craft-vp9`) use this branch. `Cargo.lock` records the
resolved commit. `TODO(#48)` in the workspace manifest still requests repointing to
`storytold/craft-libs` after merge.

## File-by-file classification

Of **16 Rust files**, **4 are byte-identical**, **1 contains a DeckCraft-only fix** and
**11 are stale or reorganized**. `Cargo.toml` is packaging, not a seventeenth Rust file.
Historical paths/line numbers refer to the deleted fork at `a77db266`.

| Fork file | Classification and disposition |
| --- | --- |
| `Cargo.toml` | Packaging: replace local `deckcraft-vp9` with `craft-vp9`; drop the local video-budget dependency from this codec. Retain the default `threads` feature in the shared crate. |
| `src/boolcoder.rs` | **Stale/local-only file:** replace the fork's permissive boolean reader with `craft-bitstream::BoolDecoder`; shared VP9 rejects malformed markers, truncation and trailing padding. |
| `src/decoder.rs` | **Fix F1/API A1:** the fork's configurable store-wide, padded-frame memory budget is not upstream. Shared VP9 instead limits luma area and adds nonempty-tile checks and parallel postfilter wavefront. |
| `src/error.rs` | **Stale:** shared crate adds typed boolean bitstream errors. |
| `src/frame.rs` | **Identical.** |
| `src/header.rs` | **Stale:** shared code reorganizes header/probability parsing, validates compressed-boolean endings and uses checked superframe offsets; fork has no exclusive header behavior to retain. |
| `src/inter.rs` | **Stale:** shared motion-compensation block descriptor and direct two-band prediction supersede the fork's arguments/copy path. |
| `src/intra.rs` | **Stale:** shared neighbor gathering and block prediction reorganize the same modes. |
| `src/lib.rs` | **Stale API:** shared `Decoder`, `Picture`, `Plane` and keyframe helpers cover DeckCraft's call sites; only the unused fork `set_memory_budget` API is missing (A1). |
| `src/loopfilter.rs` | **Identical.** |
| `src/probs.rs` | **Stale:** shared coefficient count accumulation is extracted; same probability contexts. |
| `src/spec_tables.rs` | **Stale formatting:** trailing newline differs; numeric tables unchanged. |
| `src/tables.rs` | **Identical.** |
| `src/tile.rs` | **Stale:** shared boolean decoder checks tile markers, payload exhaustion and padding. |
| `src/tile/recon.rs` | **Stale:** shared token/transform region structs and count handling replace fork's positional arguments. |
| `src/transform.rs` | **Stale:** shared inverse-transform parameter struct replaces positional arguments; math unchanged. |
| `src/transform_gen.rs` | **Identical.** |

Shared-only README, example, test support, conformance/hostile-input tests and table
generators are upstream provenance and coverage, not deleted fork files. There is no VP8
fork: `crates/media/tests/it/decode.rs` explicitly locks in `Unsupported("VP8 video")`.
`crates/ogg` demuxes Ogg audio and does not depend on VP9 or VP8; WebM/Matroska video uses
`crates/matroska` and the `crates/media/src/video.rs` codec dispatch. Neither demuxer
requires a new codec wrapper. `craft-vp8` is therefore not a DeckCraft dependency.

## Shared follow-up

| ID | DeckCraft-only behavior and upstream action |
| --- | --- |
| F1 | `src/decoder.rs:267-273,781-793`: `store_fits` checks the 64-sample-padded luma/chroma planes, eight reference slots, current and in-flight frames against a configurable byte budget before decoding. Shared `MAX_LUMA_SAMPLES` limits one frame's area instead; it does not cap the full store. Port the checked store-wide budget to `craft-vp9` with regressions for the limit and multiple in-flight frames. |
| A1 | `src/decoder.rs:76-77,109-113,132`: `set_memory_budget` and the default `deckcraft-video-budget::DECODE_MEMORY_BUDGET` exist only in the fork. No DeckCraft call site customizes this value. Decide whether the upstream API should expose a configurable budget as part of F1. |

Fork regressions `src/decoder.rs:795-815` check padded 4K/16K frames, exact byte
boundary and reference count; move equivalent tests with F1. Fixture equivalence below
does not establish hostile-input or high-resolution memory safety.

## Fixture evidence

Before and after repointing, decode all fixture RGBA frames, reverse-seek each frame at
`index * 0.1 + 0.05` seconds and render a poster PNG at each position. Compare raw bytes
of all three outputs per position, without regenerating or changing any fixture. The
existing `video_fixture_pixels_match_pre_migration` regression also checks fingerprints.
`diff -rq` compared the captured raw bytes: **84/84 outputs identical; zero
differences**.

| Fixture | RGBA frames | Reverse seeks | Poster PNGs | Result |
| --- | ---: | ---: | ---: | --- |
| `clip-h264.mp4` | 5 | 5 | 5 | Byte-identical |
| `clip-h264.mov` | 6 | 6 | 6 | Byte-identical |
| `clip-hevc.mp4` | 6 | 6 | 6 | Byte-identical |
| `clip-vp9.webm` | 5 | 5 | 5 | Byte-identical with shared VP9 |
| `clip-av1.mkv` | 6 | 6 | 6 | Byte-identical |

`clip-vp8.webm` still returns `Unsupported("VP8 video")`: there are no VP8
decoded pixels to compare. The temporary capture harness and byte files were
removed after validation; no fixture was regenerated or modified.

## Validation and scope

Use rustup stable for compiler, rustdoc and nested xtask commands, with
`CARGO_TARGET_DIR=/Users/emm/ArtCraft/worktrees/deckcraft-48-vp9/target-48` (removed after
validation). Run touched-crate tests/clippy (`--all-targets -- -D warnings`), workspace
fmt check, layers and wasm. Skip workspace-wide CI/tests/clippy/assets/PDF gates as
requested. Do not push.

- `cargo test -p deckcraft-media -p deckcraft-video-budget -p xtask`: **77 passed,
  0 failed** (media 23 unit + 22 integration; video-budget 3; xtask 29).
- `cargo test --manifest-path target-48/shared/Cargo.toml -p craft-vp9`:
  **78 passed, 0 failed, 3 ignored performance tests** (archived shared source at
  the pinned commit inside the throwaway target directory; conformance fixtures
  generated locally there using ffmpeg).
- `cargo clippy -p deckcraft-media -p deckcraft-video-budget -p xtask --all-targets -- -D warnings`:
  passed; `craft-vp9` checked as a dependency. Shared crate all-targets clippy
  also passed against the archived source.
- `cargo fmt --all -- --check`: passed.
- `cargo xtask layers`: passed, 24 crates, no violations.
- `cargo xtask wasm`: passed, 19 WASM-safe crates; existing missing sibling
  craft-fonts and UI `opening_generation` warnings remain outside this change.

Deleting the fork removes **7,681 lines in 17 files** (7,661 Rust lines plus
the manifest). No push is performed.

## Revision history

| Date | Change |
| --- | --- |
| 2026-11-03 | File-by-file audit and shared dependency migration. |
