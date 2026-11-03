//! Dependency layering rules (plan/architecture.md §3).
//!
//! The rule engine works on a small, metadata-independent model so it can be
//! unit-tested; `from_metadata` builds that model from `cargo metadata`.

use craft_xtask::layers::{Config, ExternalRule};
pub use craft_xtask::layers::{Crate, Dep, DepKind};
use serde_json::Value;

/// Where a workspace crate sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Regular layered crate.
    Layer(u8),
    /// Layer 0, and additionally may depend on no workspace crate at all.
    Standalone,
    /// Test tooling: may depend on anything up to L5 (it sits at L6 for rule
    /// purposes); other crates may use it only as a dev-dependency.
    Testkit,
    /// Binaries and build tooling: exempt from the rules.
    Exempt,
}

/// The layering table. Names are package names without the `deckcraft-`
/// prefix.
pub const TABLE: &[(&str, Class)] = &[
    ("geom", Class::Layer(0)),
    // Codecs and containers (ported from FilmCraft): L0 standalone leaves, L1 codecs over
    // `bitstream`, and `media` (probe, decode, playback mixer) at L2.
    ("bitstream", Class::Standalone),
    ("matroska", Class::Standalone),
    ("ogg", Class::Standalone),
    ("opus", Class::Standalone),
    ("h264", Class::Layer(1)),
    ("hevc", Class::Layer(1)),
    ("vp9", Class::Layer(1)),
    ("av1", Class::Layer(1)),
    ("isobmff", Class::Layer(1)),
    ("media", Class::Layer(2)),
    ("color", Class::Layer(0)),
    // WMF/EMF reader (ported from WordCraft).
    ("metafile", Class::Layer(0)),
    ("model", Class::Layer(1)),
    ("fonts", Class::Layer(1)),
    ("text", Class::Layer(2)),
    ("anim", Class::Layer(2)),
    ("format", Class::Layer(2)),
    ("render", Class::Layer(3)),
    ("pptx", Class::Layer(3)),
    ("pdf", Class::Layer(4)),
    ("engine", Class::Layer(5)),
    ("ui-egui", Class::Layer(6)),
    ("mcp", Class::Layer(6)),
    ("testkit", Class::Testkit),
    // L7 apps and tooling
    ("deckcraft", Class::Exempt),
    ("cli", Class::Exempt),
    ("web", Class::Exempt),
    ("xtask", Class::Exempt),
];

pub fn short_name(pkg: &str) -> &str {
    pkg.strip_prefix("deckcraft-").unwrap_or(pkg)
}

pub fn classify(pkg: &str) -> Option<Class> {
    TABLE.iter().find(|(name, _)| *name == short_name(pkg)).map(|(_, class)| *class)
}

/// Build the model from `cargo metadata --format-version 1 --no-deps`.
pub fn from_metadata(meta: &Value) -> Result<Vec<Crate>, String> {
    let pkgs = meta["packages"].as_array().ok_or("metadata: no packages array")?;
    let members: Vec<&str> = pkgs.iter().filter_map(|p| p["name"].as_str()).collect();
    let mut out = Vec::new();
    for p in pkgs {
        let name = p["name"].as_str().ok_or("package without name")?.to_owned();
        let mut deps = Vec::new();
        for d in p["dependencies"].as_array().into_iter().flatten() {
            let dname = d["name"].as_str().unwrap_or_default().to_owned();
            let kind = match d["kind"].as_str() {
                Some("dev") => DepKind::Dev,
                Some("build") => DepKind::Build,
                _ => DepKind::Normal,
            };
            let workspace = members.contains(&dname.as_str()) || d["path"].is_string();
            deps.push(Dep { name: dname, kind, workspace });
        }
        out.push(Crate { name, deps });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn describe(class: Option<Class>) -> String {
    match class {
        Some(Class::Layer(l)) => format!("L{l}"),
        Some(Class::Standalone) => "L0 standalone".into(),
        Some(Class::Testkit) => "testkit".into(),
        Some(Class::Exempt) => "exempt".into(),
        None => "UNREGISTERED".into(),
    }
}

pub fn config() -> Config {
    let full = |name: &str| if matches!(name, "deckcraft" | "xtask") { name.to_owned() } else { format!("deckcraft-{name}") };
    let mut config = Config { root: crate::root(), ..Config::default() };
    config.standalone = TABLE.iter().filter(|(_, class)| *class == Class::Standalone).map(|(name, _)| full(name)).collect();
    for layer in 0..=6 {
        let names = TABLE
            .iter()
            .filter(|(_, class)| match class {
                Class::Layer(value) => *value == layer,
                Class::Standalone => layer == 0,
                Class::Testkit => layer == 6,
                Class::Exempt => false,
            })
            .map(|(name, _)| full(name))
            .collect();
        config.layers.push((format!("L{layer}"), names));
    }
    for (name, class) in TABLE {
        match class {
            Class::Exempt => config.exempt.push(full(name)),
            Class::Standalone => {}
            Class::Testkit => config.testkits.push(full(name)),
            Class::Layer(_) => {}
        }
    }
    config.allowed_dev_edges.push((full("pptx"), full("engine")));
    config.external_rules.push(ExternalRule {
        dependencies: ["egui", "eframe", "winit", "egui_kittest", "rfd", "bevy*"].map(str::to_owned).to_vec(),
        allowed_from: vec!["deckcraft-ui-egui".into(), "deckcraft-mcp".into(), "deckcraft-testkit".into()],
        allowed_dev_from: vec![],
    });
    config
}

fn diagnostic(message: String) -> Option<String> {
    if let Some(name) = message.strip_suffix(": unregistered workspace crate") {
        return Some(format!("{name}: unknown workspace crate; register it in xtask/src/layers.rs TABLE (see plan/architecture.md §3)"));
    }
    let Some((source, rest)) = message.split_once(" -> ") else { return Some(message) };
    let Some((target, rule)) = rest.split_once(": ") else { return Some(message) };
    let (target, kind) = target.split_once(" [").map_or((target, ""), |(target, kind)| (target, kind.trim_end_matches(']')));
    let level = |name| match classify(name) {
        Some(Class::Layer(value)) => value,
        Some(Class::Standalone) => 0,
        Some(Class::Testkit) => 6,
        _ => 7,
    };
    Some(match rule {
        "testkit is dev-only" => format!("{source}: deckcraft-testkit may only be a dev-dependency"),
        "standalone dependency" => format!("{source}: standalone crate must not depend on workspace crate {target}"),
        "forbidden external dependency" => {
            format!("{source} (L{}) depends on UI crate `{target}`; UI toolkits are only allowed in L6+", level(source))
        }
        "disallowed layer edge" => {
            classify(target)?;
            format!("{source} (L{}) -> {target} (L{}) [{kind}]: may only depend on strictly lower layers", level(source), level(target))
        }
        _ => message,
    })
}

pub fn check(crates: &[Crate]) -> Result<Vec<String>, String> {
    let mut violations: Vec<_> = craft_xtask::layers::check(&config(), crates)?.into_iter().filter_map(diagnostic).collect();
    violations.sort();
    violations.dedup();
    Ok(violations)
}

#[cfg(test)]
#[path = "../tests/support/layers.rs"]
mod tests;
