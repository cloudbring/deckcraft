use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Tree(PathBuf);

impl Tree {
    fn new() -> Self {
        let tree = Self(std::env::temp_dir().join(format!("deckcraft-xtask-golden-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
        std::fs::create_dir(&tree.0).unwrap();
        tree.write("Cargo.toml", include_bytes!("golden/version.input.toml"));
        tree.write("member/Cargo.toml", b"[package]\nname = \"golden\"\nversion.workspace = true\nedition.workspace = true\n");
        tree.write("member/src/lib.rs", b"#[test]\nfn sample() {}\n// #[test] ignored\n");
        tree
    }

    fn write(&self, name: &str, bytes: &[u8]) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command.args(args).current_dir(&self.0).env("CARGO_MANIFEST_DIR", self.0.join("member"));
        command
    }

    fn run(&self, args: &[&str], expected: &[u8]) {
        assert_output(self.command(args).output().unwrap(), expected);
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_output(output: Output, expected: &[u8]) {
    assert!(output.status.success(), "stdout: {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, expected);
}

#[test]
fn version_stdout_manifest_and_lock_match_legacy() {
    let tree = Tree::new();
    tree.write("CHANGELOG.md", b"# Changelog\n\n## [Unreleased]\nKeep this unchanged.\n");
    tree.run(&["version"], include_bytes!("golden/version.stdout"));
    tree.run(&["version", "set", "1.2.3-rc.1"], include_bytes!("golden/version-set.stdout"));
    assert_eq!(std::fs::read(tree.0.join("Cargo.toml")).unwrap(), include_bytes!("golden/version.updated.toml"));
    assert_eq!(std::fs::read(tree.0.join("Cargo.lock")).unwrap(), include_bytes!("golden/version.updated.lock"));
    assert_eq!(std::fs::read(tree.0.join("CHANGELOG.md")).unwrap(), b"# Changelog\n\n## [Unreleased]\nKeep this unchanged.\n");
    let crlf = include_str!("golden/version.input.toml").replace('\n', "\r\n");
    assert_eq!(craft_xtask::version::replace(&crlf, "1.2.3-rc.1").unwrap(), include_str!("golden/version.updated.toml").replace('\n', "\r\n"));
}

#[test]
fn stats_stdout_and_exclusions_match_legacy() {
    let tree = Tree::new();
    for directory in ["target", "fuzz", ".hidden"] {
        tree.write(&format!("member/{directory}/ignored.rs"), b"#[test]\nfn ignored() {}\n");
    }
    tree.run(&["stats"], include_bytes!("golden/stats.stdout"));
}

#[test]
fn ico_stdout_and_bytes_match_legacy() {
    let tree = Tree::new();
    for size in [16_u32, 32, 48, 256] {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&size.to_be_bytes());
        png.extend_from_slice(&size.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0, 1, 2, 3, 4]);
        tree.write(&format!("{size}.png"), &png);
    }
    tree.run(&["ico", "out.ico", "16.png", "32.png", "48.png", "256.png"], include_bytes!("golden/ico.stdout"));
    let actual: String = std::fs::read(tree.0.join("out.ico")).unwrap().iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(actual, include_str!("golden/ico.hex").trim_end());
}

#[test]
fn current_tree_layers_match_legacy() {
    assert_output(
        Command::new(env!("CARGO_BIN_EXE_xtask")).arg("layers").env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR")).output().unwrap(),
        include_bytes!("golden/layers-repo.stdout"),
    );
}

#[test]
fn current_tree_stats_match_except_changed_xtask_and_total() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask")).arg("stats").env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR")).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let unchanged =
        |report: &str| report.lines().filter(|line| !line.starts_with("xtask ") && !line.starts_with("TOTAL ")).collect::<Vec<_>>().join("\n");
    assert_eq!(unchanged(&String::from_utf8(output.stdout).unwrap()), unchanged(include_str!("golden/stats-repo.stdout")));
}

#[test]
fn real_manifest_version_rewrite_matches_legacy() {
    let input = include_str!("golden/version-repo.input.toml");
    assert_eq!(format!("{}\n", craft_xtask::version::read(input).unwrap()), include_str!("golden/version-repo.stdout"));
    assert_eq!(craft_xtask::version::replace(input, "1.2.3-rc.1").unwrap(), include_str!("golden/version-repo.updated.toml"));
}

#[test]
fn parity_output_and_document_match_legacy() {
    let tree = Tree::new();
    tree.write("docs/parity-checklist.md", include_bytes!("golden/parity.input.md"));
    tree.run(&["parity"], include_bytes!("golden/parity.stdout"));
    assert_eq!(std::fs::read(tree.0.join("docs/parity-checklist.md")).unwrap(), include_bytes!("golden/parity.updated.md"));
    tree.run(&["parity"], include_bytes!("golden/parity.stdout"));
}

#[cfg(unix)]
fn policy_tree(metadata: &[u8]) -> Tree {
    use std::os::unix::fs::PermissionsExt;
    let tree = Tree::new();
    tree.write("metadata.json", metadata);
    tree.write("cargo.sh", include_bytes!("golden/cargo.sh"));
    std::fs::set_permissions(tree.0.join("cargo.sh"), std::fs::Permissions::from_mode(0o700)).unwrap();
    tree
}

#[cfg(unix)]
#[test]
fn forbidden_layers_match_legacy_status_and_diagnostics() {
    let tree = policy_tree(include_bytes!("golden/layers-invalid.metadata.json"));
    let output = tree.command(&["layers"]).env("CARGO", tree.0.join("cargo.sh")).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, include_bytes!("golden/layers-invalid.stdout"));
    assert_eq!(output.stderr, include_bytes!("golden/layers-invalid.stderr"));
}

#[cfg(unix)]
#[test]
fn pdf_pass_and_production_parser_failure_match_legacy() {
    let tree = policy_tree(include_bytes!("golden/pdf.metadata.json"));
    let run = || tree.command(&["pdf-deps"]).env("CARGO", tree.0.join("cargo.sh")).output().unwrap();
    assert_output(run(), include_bytes!("golden/pdf-deps.stdout"));
    let mut metadata: serde_json::Value = serde_json::from_slice(include_bytes!("golden/pdf.metadata.json")).unwrap();
    metadata["resolve"]["nodes"][0]["deps"][1]["dep_kinds"][0]["kind"] = "build".into();
    tree.write("metadata.json", &serde_json::to_vec(&metadata).unwrap());
    let output = run();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, include_bytes!("golden/pdf-reader.stderr"));
}

#[test]
fn current_tree_pdf_report_matches_legacy() {
    assert_output(
        Command::new(env!("CARGO_BIN_EXE_xtask")).arg("pdf-deps").env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR")).output().unwrap(),
        include_bytes!("golden/pdf-deps-repo.stdout"),
    );
}

#[test]
fn help_keeps_local_text_and_shared_syntax() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask")).arg("--help").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for task in [
        "assets",
        "bench",
        "ci",
        "wasm",
        "parity",
        "pdf-deps",
        "layers",
        craft_xtask::version::cli_help(),
        craft_xtask::stats::cli_help(),
        craft_xtask::ico::cli_help(),
    ] {
        assert!(text.contains(task), "{task}");
    }
}
