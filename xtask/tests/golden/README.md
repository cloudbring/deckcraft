# Legacy task goldens

Captured on 2026-11-03 from DeckCraft main
`3e178d8f5a59c787a6a53a7ed0f052b3e4a1489f`, before deleting the task
implementations. The only preliminary source edit enabled runtime
`CARGO_MANIFEST_DIR` root selection for disposable test trees, following
LightCraft `241b4be9`; it did not change task algorithms.

- `*-repo.stdout`: version, statistics, layers and PDF dependency output on the
  current workspace. The statistics comparison excludes only the changed xtask
  row and its total. Its captured xtask count includes the two-line root helper
  expansion, before integration tests were added.
- `version.*`: old command stdout and actual manifest/lockfile edits in a tiny
  disposable workspace, starting at DeckCraft's current version. The Cargo
  update is real and offline; no application package is built.
- `version-repo.*.toml`: copies of the real root manifest and its expected
  version-only edit, tested independently of live release/version changes.
- `parity.*`: the repository's real checklist, the old command's stdout and
  rewritten file, captured on a temporary copy, never on the live document.
- `ico.*`: old command output and packed bytes in hexadecimal, using four
  synthetic IHDR buffers (16, 32, 48, 256 px), not third-party artwork.
- `stats.stdout`: old command output on a small source tree containing excluded
  target, fuzz and hidden directories.
- `layers-invalid.*`, `pdf*`, `cargo.sh`: synthetic metadata and controlled
  Cargo tree output for legacy success/failure cases. The shell stub performs
  no build; real current-tree metadata/tree checks supplement these fixtures.

All task fixtures are first-party output or contributor-original synthetic
data. They fit below the 200 KB budget. Fixtures are immutable observations,
not snapshots regenerated from the new implementation. Version stdout and file
bytes are asserted; Cargo's non-stable progress stderr is not a golden.
Layer stdout intentionally includes padded empty columns; `.gitattributes`
preserves these legacy trailing spaces rather than treating them as errors.
