//! Check the immutable writer sources and keep PDF reading out of production exports.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

const SHARED_REV: &str = "c5a1cbfb5f9049f9270955caffb5bee5997aebf7";
const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

pub fn run(root: &Path) -> Result<(), String> {
    let policy = config(root);
    let output =
        policy.cargo.command().args(["metadata", "--locked", "--format-version", "1"]).output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    craft_xtask::pdf_deps::validate(&policy, &metadata)?;
    // Metadata merges target features even with --filter-platform. Check Cargo's
    // selected feature graph separately so native compression never enables threads on WASM.
    let output =
        Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into())).arg("-vV").output().map_err(|e| format!("rustc host: {e}"))?;
    if !output.status.success() {
        return Err(format!("rustc host: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let version = String::from_utf8(output.stdout).map_err(|e| format!("rustc host: {e}"))?;
    let host = version.lines().find_map(|line| line.strip_prefix("host: ")).ok_or("rustc did not report its host target")?;
    for rule in &policy.target_features {
        let target = rule.target.as_deref().unwrap_or(host);
        let output = policy
            .cargo
            .command()
            .args([
                "tree",
                "--locked",
                "--target",
                target,
                "--edges",
                "normal,build",
                "--package",
                &rule.root_package,
                "--depth",
                &rule.depth.to_string(),
                "--no-dedupe",
                "--prefix",
                "none",
                "--format",
                "{p}|{f}",
            ])
            .output()
            .map_err(|e| format!("cargo tree ({target}): {e}"))?;
        if !output.status.success() {
            return Err(format!("cargo tree ({target}): {}", String::from_utf8_lossy(&output.stderr)));
        }
        validate_target_features(rule, &String::from_utf8_lossy(&output.stdout)).map_err(|e| format!("{target}: {e}"))?;
    }
    println!("PDF writer: pinned craft-pdf/Krilla; native-only parallel compression; parser assertions are development-only");
    Ok(())
}

pub fn config(root: &Path) -> craft_xtask::pdf_deps::Config {
    use craft_xtask::pdf_deps::{Config, PackagePin, TargetFeatures, git_source};
    let source = git_source("https://github.com/storytold/craft-libs", SHARED_REV);
    let packages = [
        ("craft-pdf", "0.1.0", source.as_str(), vec!["write", "write-simple-text"]),
        ("krilla", "0.8.2", source.as_str(), vec!["raster-images", "rayon", "simple-text"]),
        ("hayro-syntax", "0.7.2", REGISTRY, vec!["std"]),
    ]
    .into_iter()
    .map(|(name, version, source, features)| PackagePin {
        name: name.into(),
        version: version.into(),
        source: source.into(),
        required_features: vec![],
        exact_features: Some(features.into_iter().map(str::to_owned).collect()),
    })
    .collect();
    Config {
        cargo: craft_xtask::Cargo { root: root.into(), ..Default::default() },
        packages,
        production_roots: vec!["deckcraft-pdf".into()],
        forbidden_production: vec!["hayro".into(), "hayro-*".into()],
        target_features: [None, Some("wasm32-unknown-unknown")]
            .into_iter()
            .map(|target| TargetFeatures {
                root_package: "deckcraft-pdf".into(),
                target: target.map(str::to_owned),
                package: "krilla".into(),
                depth: 2,
                features: if target.is_none() { vec!["raster-images", "rayon", "simple-text"] } else { vec!["raster-images", "simple-text"] }
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            })
            .collect(),
        ..Config::default()
    }
}

fn validate_target_features(rule: &craft_xtask::pdf_deps::TargetFeatures, tree: &str) -> Result<(), String> {
    craft_xtask::pdf_deps::validate_target_features(rule, tree).map_err(|error| {
        if error == "missing krilla in target dependency graph" {
            "missing Krilla in the exporter's target dependency graph".into()
        } else {
            error.replace(", got ", "; got ")
        }
    })
}

#[cfg(test)]
#[path = "../tests/support/pdf_deps.rs"]
mod tests;
