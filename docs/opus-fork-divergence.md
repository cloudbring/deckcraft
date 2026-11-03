# Opus fork divergence — algorithms #48, step 3

> **Last reviewed / updated:** 2026-11-03 · **Status:** consumer migrated; Ogg retained.

## Scope and reproducible inputs

Compared every file in DeckCraft `crates/opus` at `a77db266` against
`cloudbring/craft-libs`, branch `algo/48-craft-opus`, commit
`26886531c1962f8aec8c5593fd8fec30e2103f0b`. The shared source comes from FilmCraft
`a29f2893f1aa8426c57dcd0bac5bd958dd440883`, including #49's MDCT-on-`craft-fft`.
The Ogg comparison uses FilmCraft's `crates/ogg` at that same source commit;
`craft-opus` does not provide an Ogg demuxer.

The prepared branch `algo/48-use-craft-opus` and its intended VP9 parent
`algo/48-use-craft-vp9` were both still at `a77db266` when work began. The VP9
migration commit was absent, so this commit builds on the supplied AV1 head,
not on uncommitted work in another agent's checkout. `plan/STATUS.md` and
`plan/execution-plan.md` were absent from this worktree.

The audit used raw diffs, copies normalized with DeckCraft's rustfmt configuration,
and the fork's history, including header fix `3ed12de9` and refactor `c58f19f1`.
No source from third-party decoder implementations was inspected.

## File-by-file Opus classification

Of **16 Rust source files**, **2 are byte-identical**, **3 differ only in
formatting**, **9 are stale implementations/refactors**, **1 contains a
superseded hardening fix**, and **1 differs only by DeckCraft regression tests**.
There are **0 DeckCraft-only runtime fixes missing from shared Opus** and no
DeckCraft-only codec features to carry forward. The manifest is a separate
packaging change: **17 fork files / 6,581 lines deleted** (6,569 Rust lines).

| Fork file | Classification | Evidence and disposition |
| --- | --- | --- |
| `Cargo.toml` | Packaging | Replace workspace package `deckcraft-opus` with `craft-opus`; shared dependencies are `craft-fft` and `craft-bitstream`. |
| `src/lib.rs` | Stale / integration | Shared version documents the FFT dependency, uses stricter lint declarations, and adds `Error::Transform` plus its conversion. All existing error variants and decoder exports remain. |
| `src/packet.rs` | Identical | Packet parsing, TOC, duration handling and tests match byte-for-byte. |
| `src/range.rs` | Identical | Opus bidirectional range decoder and test encoder match byte-for-byte. |
| `src/header.rs` | Superseded fix | DeckCraft #425 validates hand-built headers. Shared `validate()` preserves every invariant and additionally bounds channels/streams, checks addition, enforces family 0/1/3 constraints and validates parsed headers. No fix is lost. |
| `src/multistream.rs` | Test-only difference | Production code, including `head.validate()?` before allocation, is identical. Two DeckCraft tests cover five invalid and four valid mappings; preserved in `crates/media/tests/it/opus.rs`. |
| `src/decoder.rs` | Stale / refactor | Equivalent SILK, fade and redundancy logic is organized differently. Shared construction propagates transform errors and caps concealment/FEC at 120 ms before resizing output. Output gain arithmetic is unchanged. |
| `src/celt.rs` | Stale / optimization | Shared version uses fallible FFT scratch/plans, argument structs and 48 kHz mono/stereo deemphasis fast paths. Silence, dynamic allocation, synthesis, loss concealment and postfilter helper extraction differs; no unique fork fix. |
| `src/celt/bands.rs` | Stale / optimization | Shared version precomputes the bounded PVQ count table once with u32 saturation; the fork rebuilds a u64 table per decode. Parameter structs and theta-helper organization differ. Shared table/reference-count regression is new. |
| `src/celt/energy.rs` | Stale / refactor | Final energy inputs are bundled into `EnergyFinalise`; arithmetic is unchanged. |
| `src/celt/mdct.rs` | Stale / replaced algorithm | Deletes the private complex FFT, mixed-radix recursion and IMDCT. Shared adapter owns four `craft-fft` plans/scratch buffers, unscales/unwindows the middle half and applies the CELT overlap rotation. New direct-formula, allocation and invalid-argument tests cover all shifts/strides. |
| `src/celt/rate.rs` | Stale / refactor | Removes `Mode.mdct`, now decoder-owned shared plans; allocation inputs become structs. Explicit arithmetic parentheses preserve operator precedence. |
| `src/celt/tables.rs` | Formatting only | Identical after applying the same rustfmt configuration; constants unchanged. |
| `src/silk.rs` | Stale / refactor | Equivalent channel setup, packet flags, LBRR skipping, stereo prediction, side-channel reset and output resampling are grouped into different helpers. |
| `src/silk/lpc.rs` | Formatting only | Line wrapping only; identical after rustfmt normalization. |
| `src/silk/resampler.rs` | Stale / optimization | Shared resampler adds eight independent dot-product accumulators and incremental phase stepping, plus a sequential-sum regression. Reassociated floating-point sums can round differently. |
| `src/silk/tables.rs` | Formatting only | Identical after rustfmt normalization; constants unchanged. |

The shared crate also has **9 files absent from the fork**: `README.md`,
`rustfmt.toml`, `test-support/mod.rs`, `test-support/vectors.sha256`, and
`tests/it/{main.rs,common/mod.rs,oracle.rs,robustness.rs,vectors.rs}`.
These add provenance, formatting, external-oracle/vector support and robustness
coverage; none is a DeckCraft-only omission requiring a new local decoder.

## Ogg stays: demuxing and DeckCraft-only fixes

All **3 local Ogg files are retained unchanged**. Relative to FilmCraft, the
manifest differs in packaging, `src/lib.rs` carries unique hardening, and
`src/tests.rs` carries its regressions (0 byte-identical files). FilmCraft's
additional `README.md` is documentation, not decoder functionality.

| Retained file | DeckCraft-specific behavior |
| --- | --- |
| `crates/ogg/Cargo.toml` | Standalone `deckcraft-ogg`, workspace lints, no runtime dependencies. |
| `crates/ogg/src/lib.rs` | Checked `OpusTiming::of` arithmetic returns `Result` for granule/duration/pre-skip overflow. A reusable 64 KiB `CaptureScanner` bounds recovery reads to a 32 MiB budget (FilmCraft uses a different 64 MiB recovery strategy). Warning collection is capped at 256 plus an omission marker, with the budget-spent warning emitted once and last. |
| `crates/ogg/src/tests.rs` | Retains timing-overflow, bounded-recovery-read, warning-cap, uncharged-valid-pages and sub-four-byte-budget regressions, in addition to CRC/packet/timing/corruption coverage. |

Removing Ogg would remove page/CRC parsing, packet reassembly, stream selection,
pre-skip/granule trimming and the protections above. The shared decoder operates
on Opus headers/packets, not Ogg pages. Both media consumers continue using local
Ogg timing and bounded output accumulation; only their decoder/probe imports change.

## Fixture parity

Decoded the existing contributor-generated fixtures with the original media
consumer before migration, then again after migration, using a temporary example
that wrote every interleaved sample as little-endian f32. Compared **all 48,648
samples**, checked equal lengths and a maximum absolute difference of `1e-6`.
The existing frequency/RMS/duration acceptance tests were not loosened.

| Fixture | Rate / channels | Samples | Changed f32 values | Maximum absolute difference | RMS difference |
| --- | --- | ---: | ---: | ---: | ---: |
| `tone.opus` | 48,000 Hz / mono | 24,000 | 14,834 | `4.470348358154297e-8` | `7.29271309852793e-9` |
| `clip-vp9.webm` audio | 48,000 Hz / mono | 24,648 | 15,280 | `4.470348358154297e-8` | `7.197587259948563e-9` |

Neither PCM output is byte-identical; both are comfortably within `1e-6`.
Probe results are unchanged: Ogg 500 ms, WebM 508 ms, both Opus/48 kHz/mono.
Differences are consistent with the shared MDCT and SILK resampler rounding
changes, not attributed to MDCT alone. Permanent regression coverage checks
sample counts, finiteness and pre-migration sample anchors (including both tails)
at the same `1e-6` tolerance; the exhaustive comparison was performed during this
migration, not by the sparse anchor regression.

Raw PCM SHA-256 evidence:

| Fixture | Before | After |
| --- | --- | --- |
| `tone.opus` | `b9b1adea401010b204263759ef48a1533a6bc645ff7621247aaab584769ae2b3` | `b8c1694d7e118221edc85841a60c54cdd203a74f228bdcae4e45e3a7e70c84dc` |
| `clip-vp9.webm` | `e5dec7f2360ae922cee567acf13ec215dd624eda5da4c99c57ae7e417a9c535e` | `57a387f7f86adb6222102cc069e19379ba8caf36ac41f79da9e972dc36c17fbc` |

The existing video golden test also passed: all 84 frame/seek/poster fingerprints
across H.264 MP4/MOV, HEVC, VP9 and AV1 are unchanged. No fixture assets changed.

## Validation

Used rustup stable tools (compiler, rustdoc, clippy, rustfmt) with
`CARGO_TARGET_DIR=/Users/emm/ArtCraft/worktrees/deckcraft-48-opus/target-48`.

- Baseline: `cargo test -p deckcraft-opus -p deckcraft-ogg -p deckcraft-media -p xtask`:
  **111 passed, 0 failed** (Opus 27, Ogg 10, media 23 unit + 22 integration, xtask 29).
- Migrated: `cargo test -p deckcraft-media -p deckcraft-ogg -p xtask`:
  **87 passed, 0 failed** (media 23 unit + 25 integration, Ogg 10, xtask 29); doctests 0/0.
- `cargo clippy -p deckcraft-media -p deckcraft-ogg -p xtask --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo xtask layers`: passed, **24 crates**, no violations.
- `cargo xtask wasm`: passed, **19 WASM-safe crates**, including transitive Opus/FFT.
- Shared Opus: the exact archived crate source, unchanged, in a temporary minimal
  workspace with dependencies at `26886531`, inherited shared lints and the shared
  Opus opt-level-3 profile: **39 passed, 0 failed, 1 ignored** (30 unit and 9
  integration tests; the ignored test is an opt-in vector diagnostic). All-targets
  clippy passed. Ran `cargo test --manifest-path target-48/audit/Cargo.toml -p craft-opus -- --nocapture`
  with `CRAFT_MEDIA_REQUIRE_ORACLES=1` and
  `CRAFT_MEDIA_FIXTURES_DIR` pointing to the temporary fixture directory.
- Downloaded the official `opus_testvectors-rfc8251.tar.gz` from
  `https://opus-codec.org/static/testvectors/`, verified its SHA-256 against the
  shared crate's `test-support/vectors.sha256`, and passed all **36 vector
  comparisons**, including exact per-packet range states. FFmpeg oracle,
  robustness and quality-calibration tests also passed. An initial optional
  conformance attempt failed because vectors were absent; the complete rerun
  after downloading them resolved that fixture-setup failure.
- Final migrated and shared suites combined: **126 passed, 0 failed, 1 ignored**.

Workspace-wide CI/tests/clippy, assets and PDF dependency gates were intentionally
skipped as requested; fmt and the explicitly requested layers/wasm gates still ran.
The existing wasm UI `opening_generation` warning and unused egui patch warnings
are outside this change. Temporary test sources, PCM dumps, downloaded vectors
and the entire `target-48` directory are removed after validation.

## Final integration / repoint

`craft-opus` and `craft-fft` are pinned to `algo/48-craft-opus` at `26886531` in
the lockfile. Other existing video/bitstream pins are deliberately untouched;
this leaves separate branch-qualified bitstream/media dependency instances until
integration. No VP9/VP8 shared pins were present in the supplied base, and the
local VP9 crate is not removed by this Opus-only commit.

When combining with the sibling VP9 migration, **leave `craft-vp9` / `craft-vp8`
on `algo/48-craft-vp9`**: the Opus branch contains the hardened H.264/HEVC/AV1 and
FFT/bitstream integration but **does not contain VP9/VP8**. Repoint all shared
codec dependencies to the final merged revision only after both branches land,
then deduplicate their branch-qualified dependencies. No Opus runtime fixes
remain to upstream; the retained Ogg hardening belongs to a separate demuxer
consolidation. No push is performed.

## Revision history

| Date | Change |
| --- | --- |
| 2026-11-03 | Audited all Opus/Ogg fork files, removed 6,581 Opus lines, retained header regressions/Ogg and measured PCM differences. |
