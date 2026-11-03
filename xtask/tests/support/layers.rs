use super::*;
use DepKind::*;

fn graph(name: &str, dependency: &str, kind: DepKind, workspace: bool) -> Vec<Crate> {
    vec![Crate { name: name.into(), deps: vec![Dep { name: dependency.into(), kind, workspace }] }]
}

#[test]
fn layer_edges_and_dev_exception() {
    for kind in [Normal, Dev, Build] {
        assert!(check(&graph("deckcraft-engine", "deckcraft-model", kind, true)).unwrap().is_empty());
        assert!(!check(&graph("deckcraft-model", "deckcraft-engine", kind, true)).unwrap().is_empty());
        assert!(!check(&graph("deckcraft-text", "deckcraft-anim", kind, true)).unwrap().is_empty());
        assert!(!check(&graph("deckcraft-geom", "deckcraft-color", kind, true)).unwrap().is_empty());
        assert_eq!(check(&graph("deckcraft-pptx", "deckcraft-engine", kind, true)).unwrap().is_empty(), kind == Dev);
    }
}

#[test]
fn standalone_and_testkit_rules() {
    for name in ["bitstream", "matroska", "ogg", "opus"] {
        for kind in [Normal, Dev, Build] {
            assert!(!check(&graph(&format!("deckcraft-{name}"), "deckcraft-geom", kind, true)).unwrap().is_empty());
            assert!(!check(&graph(&format!("deckcraft-{name}"), "deckcraft-testkit", kind, true)).unwrap().is_empty());
        }
    }
    assert!(check(&graph("deckcraft-render", "deckcraft-testkit", Dev, true)).unwrap().is_empty());
    assert!(!check(&graph("deckcraft-render", "deckcraft-testkit", Normal, true)).unwrap().is_empty());
    assert!(check(&graph("deckcraft-testkit", "deckcraft-engine", Normal, true)).unwrap().is_empty());
    assert!(!check(&graph("deckcraft-testkit", "deckcraft-ui-egui", Normal, true)).unwrap().is_empty());
}

#[test]
fn external_ui_boundary() {
    for dependency in ["egui", "eframe", "winit", "egui_kittest", "rfd", "bevy_ecs", "bevy"] {
        for kind in [Normal, Dev, Build] {
            assert!(!check(&graph("deckcraft-engine", dependency, kind, false)).unwrap().is_empty());
            assert!(check(&graph("deckcraft-mcp", dependency, kind, false)).unwrap().is_empty());
            assert!(check(&graph("deckcraft-testkit", dependency, kind, false)).unwrap().is_empty());
        }
    }
    assert!(check(&graph("deckcraft-engine", "egui_extras_not", Normal, false)).unwrap().is_empty());
}

#[test]
fn exemptions_self_and_unknown_dependencies() {
    for name in ["deckcraft", "deckcraft-cli", "deckcraft-web", "xtask"] {
        assert!(check(&graph(name, "deckcraft-ui-egui", Normal, true)).unwrap().is_empty());
        assert!(!check(&graph("deckcraft-model", name, Normal, true)).unwrap().is_empty());
    }
    assert!(check(&graph("deckcraft-model", "deckcraft-model", Dev, true)).unwrap().is_empty());
    assert!(check(&graph("deckcraft-model", "deckcraft-unknown", Normal, true)).unwrap().is_empty());
    assert!(!check(&graph("deckcraft-unknown", "deckcraft-model", Normal, true)).unwrap().is_empty());
}

#[test]
fn path_dependencies_preserve_legacy_workspace_classification() {
    let metadata = serde_json::json!({"packages":[{"name":"deckcraft-model","dependencies":[
        {"name":"deckcraft-engine","kind":"build","path":"/outside/workspace"},
        {"name":"serde","kind":null}
    ]}]});
    let crates = from_metadata(&metadata).unwrap();
    assert!(crates[0].deps[0].workspace);
    assert!(!crates[0].deps[1].workspace);
    assert_eq!(crates[0].deps[0].kind, Build);
    assert_eq!(check(&crates).unwrap().len(), 1);
}
