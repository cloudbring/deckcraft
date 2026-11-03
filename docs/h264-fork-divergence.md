# H.264 fork migration audit (#48)

Compared `deckcraft` at the parent of this migration with `cloudbring/craft-libs` branch
`algo/48-craft-h264` at `935b3523`. Both copies descend from FilmCraft, but the shared
crate is a later and substantially different implementation. No fork source is retained.

## Per-file classification

Five identical files: `cabac.rs`, `cabac_tables.rs`, `cavlc_tables.rs`, `mbtypes.rs`,
`tables.rs`. Fifteen drifted files (the categories below account for all fifteen):

| DeckCraft fork file | Divergence and disposition |
| --- | --- |
| `cabac_mb.rs` | Stale macroblock-completion error path and formatting; use shared implementation. |
| `cavlc.rs` | Crate-name imports only; use shared implementation. |
| `deblock.rs` | Stale filter decomposition, boundary-strength helpers and formatting; use shared implementation. |
| `decoder.rs` | **Upstream fixes needed:** transactional/typed `avcC` SPS/PPS validation, whole-picture-store budget, and support for valid frame-number gaps greater than 16. Other worker/DPB and picture-boundary drift is stale; use shared decoder. |
| `dpb.rs` | Older non-generic DPB and gap-fill implementation; shared DPB already has bounded large-gap materialization and wrap tests. |
| `error.rs` | Crate-name import only. |
| `inter.rs` | Older motion-compensation helpers and call shape; use shared implementation. |
| `intra.rs` | Prediction helper reshuffle and formatting; use shared implementation. |
| `lib.rs` | Crate name, exports and documentation; use shared implementation. |
| `params.rs` | Cropping checked with `u64` instead of shared checked `u32`; shared check is already overflow-safe. DeckCraft's `pic_init_qs_minus26` lower bound is too permissive for high bit depth; keep the shared range. Remaining parser/test and formatting drift is stale. |
| `picture.rs` | Older row/picture access formatting and window access; use shared implementation. |
| `slice.rs` | **Upstream fix needed:** `checked_add` when combining `pic_init_qp` and `slice_qp_delta` before checking range; shared arithmetic can overflow on hostile Exp-Golomb values. `qp()` in the fork saturates, but the checked parse is the fix to port. Other parser refactoring is stale. |
| `slicedec.rs` | **Upstream fix needed:** temporal-direct MV scaling in `i32`, clipping both results to `i16` range, deriving L1 from unclipped L0 (including `dsf=512, mvCol=32767`). Shared's `as i16` wraps, and L1 subtraction can overflow. Remaining per-block reconstruction and decomposition drift is stale. |
| `synth_tests.rs` | Fork-specific tests for `avcC` validation, whole-store budget, gaps and weighted/PCM decoding; port regression coverage along with fixes. Shared has its own oracle-based suite. |
| `transform.rs` | **Upstream fix needed:** clamp coefficient `r` to `[-32768, 32767]` before 4×4 and 8×8 inverse transform butterflies; unbounded dequantized levels can overflow shared `i32` additions. Other formatting is stale. |

The `deckcraft-bitstream` fork was a `craft-media::bitstream` re-export plus the local
`budget.rs`. The re-export is replaced by `craft-bitstream`; its DeckCraft-only
`PictureShape`/`DECODE_MEMORY_BUDGET` remains in `deckcraft-video-budget` for HEVC,
VP9 and AV1. The deleted fork's H.264 whole-store budget check must be ported to
`craft-h264` itself; this local helper alone does not protect the shared decoder.

## Behavior and follow-up

`deckcraft-media` adapts the shared H.264 decoder at its existing codec boundary;
MP4/MOV import, frame decoding, seek, poster PNG, video feed and media-command
poster generation all use it. The original fork's MP4 and MOV fixture RGBA frame
hashes, seek hashes and poster-PNG hashes are recorded in the integration test,
and the shared decoder passes them unchanged. These two fixtures do not exercise
the hostile-input fixes or large-gap behavior above. Port those fixes and tests
to the shared crate in a separate craft-libs change before treating #48 as
fully closed. Do not reintroduce an H.264 fork to paper over them.
