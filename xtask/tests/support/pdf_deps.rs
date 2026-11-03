use super::*;
use serde_json::json;

fn validate(metadata: &Value) -> Result<(), String> {
    craft_xtask::pdf_deps::validate(&config(Path::new(".")), metadata)
}

fn validate_target_features(tree: &str, parallel: bool) -> Result<(), String> {
    let policy = config(Path::new("."));
    let rule = policy.target_features.iter().find(|rule| rule.target.is_none() == parallel).unwrap();
    super::validate_target_features(rule, tree)
}

fn metadata() -> Value {
    let source = format!("git+https://github.com/storytold/craft-libs?rev={SHARED_REV}#{SHARED_REV}");
    json!({
        "packages": [
            {"id":"app", "name":"deckcraft-pdf"},
            {"id":"writer", "name":"craft-pdf", "version":"0.1.0", "source":source},
            {"id":"backend", "name":"krilla", "version":"0.8.2", "source":source},
            {"id":"parser", "name":"hayro-syntax", "version":"0.7.2", "source":REGISTRY}
        ],
        "resolve":{"nodes":[
            {"id":"app", "deps":[{"pkg":"writer", "dep_kinds":[{"kind":null}]}, {"pkg":"parser", "dep_kinds":[{"kind":"dev"}]}]},
            {"id":"writer", "features":["write","write-simple-text"], "deps":[{"pkg":"backend", "dep_kinds":[{"kind":null}]}]},
            {"id":"backend", "features":["raster-images","rayon","simple-text"], "deps":[]},
            {"id":"parser", "features":["std"], "deps":[]}
        ]}
    })
}

#[test]
fn parallel_compression_is_required_only_on_native_targets() {
    let serial = "krilla v0.8.2 (pinned)|raster-images,simple-text\n";
    let parallel = "krilla v0.8.2 (pinned)|raster-images,rayon,simple-text\n";
    assert!(validate_target_features(serial, false).is_ok());
    assert!(validate_target_features(parallel, true).is_ok());
    assert!(validate_target_features(serial, true).is_err());
    assert!(validate_target_features(parallel, false).is_err());
    assert!(validate_target_features("", false).is_err());
}

#[test]
fn rejects_unpinned_or_duplicate_writer_sources() {
    assert!(validate(&metadata()).is_ok());
    for source in [Value::Null, json!(REGISTRY), json!("git+https://github.com/storytold/craft-libs#other")] {
        let mut m = metadata();
        m["packages"][1]["source"] = source;
        assert!(validate(&m).is_err());
    }
    let mut m = metadata();
    let duplicate = m["packages"][2].clone();
    m["packages"].as_array_mut().unwrap().push(duplicate);
    assert!(validate(&m).is_err());
}

#[test]
fn rejects_import_features_and_missing_text_shaping() {
    for (node, features) in [
        (1, json!([])),
        (1, json!(["write"])),
        (1, json!(["write", "write-simple-text", "write-pdf"])),
        (2, json!(["raster-images", "rayon", "simple-text", "pdf"])),
        (2, json!(["raster-images"])),
        (3, json!(["std", "images"])),
    ] {
        let mut m = metadata();
        m["resolve"]["nodes"][node]["features"] = features;
        assert!(validate(&m).is_err());
    }
}

#[test]
fn rejects_dev_parser_promoted_to_production_directly_or_transitively() {
    for kind in [Value::Null, json!("build")] {
        let mut m = metadata();
        m["resolve"]["nodes"][0]["deps"][1]["dep_kinds"][0]["kind"] = kind;
        assert!(validate(&m).is_err());
    }
    let mut m = metadata();
    m["resolve"]["nodes"][2]["deps"] = json!([{"pkg":"parser", "dep_kinds":[{"kind":null}]}]);
    assert!(validate(&m).is_err());
}

#[test]
fn rejects_incomplete_dependency_metadata() {
    let mut m = metadata();
    m["resolve"]["nodes"][2]["deps"] = json!([{"pkg":"missing", "dep_kinds":[{"kind":null}]}]);
    assert!(validate(&m).is_err());
    let mut m = metadata();
    m["resolve"] = Value::Null;
    assert!(validate(&m).is_err());
    let mut m = metadata();
    m["packages"].as_array_mut().unwrap().retain(|p| p["name"] != "hayro-syntax");
    assert!(validate(&m).is_err());
}
