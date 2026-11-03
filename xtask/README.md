# DeckCraft workspace tasks

Ticket #70 delegates reusable tooling to `craft-xtask`, locked to craft-libs
`89133cd7c5d061ebd6ffcb933dd786c42794f07c` on `algo/integration`.
Only this development crate depends on the library; runtime packages are unchanged.

| Task | Shared implementation | Local compatibility policy |
|---|---|---|
| version | `version::run` | Default root manifest, staging, offline refresh and online fallback |
| stats | `stats::run` | Follow symlinks as before; retain legacy `--exact` detection |
| ico | `ico::run` | Paths remain relative to the caller; arbitrary ordered square PNGs up to 256 px, no fixed sizes |
| layers | `layers::check` and graph types | DeckCraft table, dev exception, path-dependency classification, report and diagnostics |
| pdf-deps | `pdf_deps::validate`, `validate_target_features` | Exact package pins/features, normal/build reachability, native/wasm trees, legacy command errors and success message |
| parity | `parity::rewrite`, `rows`, `summary` | Existing checklist markers and summary-only stdout |

`assets` and `bench` remain app-owned. Existing CI, test, doctest and wasm
orchestration also remains local; the wasm package selection still uses the
same layer table. The original help text is retained, followed by shared syntax
for the tasks directly dispatched to `run`.

## Why reporting adapters remain

Like LightCraft consumer commit `241b4be9`, DeckCraft cannot use `layers::run`
without changing report text. The shared metadata parser also treats external
path dependencies differently. The local adapter retains that classification
and translates shared diagnostics, but does not duplicate the rule engine.

`parity::run` prints the complete rewritten checklist, whereas DeckCraft prints
only the summary. `pdf_deps::run` prints a generic success message and uses
different command-error prefixes. These remain small adapters over public shared
functions, rather than replacing observable output. PDF target/dependency policy
is configured once, with native-only `rayon` and dev-only `hayro*` reachability.

## Behavior boundaries

Current-tree layers/PDF results and text match main; parity stdout and rewritten
content, ordinary statistics, version stdout/edits and ICO bytes have legacy
goldens. Shared version updates capture Cargo output instead of streaming its
progress to stderr, and add `version tag`. Invalid version/ICO usage can use
shared error wording. Shared PDF validation retains rejection policy but some
invalid feature/metadata diagnostics differ. Reversed parity markers now return
an error instead of producing a malformed document. Stats traversal is now
bounded and cycle-safe rather than unbounded; ordinary acyclic symlinks remain
followed. These are not claims of identical behavior for malformed inputs.

## Verification

Run only the `xtask` package when validating this migration. Export the assigned
`CARGO_TARGET_DIR` before Cargo commands. `cargo test -p xtask` exercises local
policy tests and `tests/golden.rs`; its real-workspace checks invoke Cargo
metadata/tree, not builds of application packages. The PDF check may require
cached workspace dependencies or network access. No `stats --exact` invocation
against the application workspace is needed.

`CARGO_MANIFEST_DIR` is resolved at runtime when it names a manifest, with a
compile-time fallback, following LightCraft's isolated golden-test pattern.
Tests set it per subprocess, never mutate process-global environment variables,
and use disposable directories for tasks that write files.
