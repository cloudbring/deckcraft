//! Old-vs-new differential for the craft-ooxml swap (#122), artcraft-algorithms#184.
//!
//! OLD = `pptx_old` (deckcraft-pptx at 3e178d8f5, the pre-swap upstream main), NEW = the current
//! `deckcraft-pptx` path crate. Both run in this one binary over every .pptx/.pptm/.potx file in
//! the Office corpus and are judged per file on:
//!   1. open ok / error equally (craft-oracle OnlyOldFails / OnlyNewFails / BothFail),
//!   2. the semantic model (`{:?}` of the parsed `Presentation`, media bytes digested) and the list
//!      of slides skipped as unreadable,
//!   3. open -> save: same part list, same part order, every XML/rels part equal as canonical XML
//!      (craft-compare), every other part byte-equal,
//!   4. the model re-read from the saved package.
//!
//! Run:  DECKCRAFT_OFFICE_CORPUS=/path/to/corpus cargo test --release -p deckcraft-ooxml-oracle \
//!         --test ooxml_oracle -- --ignored --nocapture
//! The corpus root holds `office-poi`, `office-openxml-sdk`, `office-libreoffice`. Expected
//! differences live in `oracle/expected.toml`. Output goes to `$DECKCRAFT_ORACLE_OUT` (default
//! `target/oracle`): `report.json`, `report.md`, `details.json`, `summary.md`.

use craft_compare::{Verdict, xml::compare_xml};
use craft_oracle::{Differential, FnJudge, FnSide, Outcome, Policy, Registry};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const CORPORA: [&str; 3] = ["office-poi", "office-openxml-sdk", "office-libreoffice"];
const EXTS: [&str; 4] = ["pptx", "pptm", "potx", "ppsx"];

/// What one side produced for one file.
struct Run {
    /// fnv1a of the input, to key `DETAILS`.
    key: u64,
    skipped: Vec<String>,
    dump: String,
    saved: Result<Vec<u8>, String>,
    reread: Result<String, String>,
}

static DETAILS: Mutex<BTreeMap<u64, Vec<String>>> = Mutex::new(BTreeMap::new());

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325, |h, &c| (h ^ u64::from(c)).wrapping_mul(0x100000001b3))
}

macro_rules! dump {
    ($p:expr) => {{
        let p = &mut $p;
        let digests: Vec<String> =
            p.media.iter().map(|m| format!("{:?}|{}|{}|{}|{:x}|{:?}", m.id, m.name, m.content_type, m.data.len(), fnv(&m.data), m.link)).collect();
        for m in &mut p.media {
            m.data = Default::default();
        }
        format!("{p:?}\nMEDIA {digests:?}")
    }};
}

macro_rules! side {
    ($krate:ident) => {
        |bytes: &[u8]| -> Result<Run, String> {
            let (mut p, skipped) = $krate::import_checked(bytes).map_err(|e| e.to_string())?;
            let saved = $krate::export(&p).map_err(|e| format!("save: {e}"));
            let reread = match &saved {
                Ok(s) => $krate::import_checked(s).map(|(mut q, sk)| format!("{}\nSKIPPED {sk:?}", dump!(q))).map_err(|e| e.to_string()),
                Err(e) => Err(e.clone()),
            };
            Ok(Run { key: fnv(bytes), skipped, dump: dump!(p), saved, reread })
        }
    };
}

fn first_diff(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()))
}

fn ctx(s: &str, at: usize) -> String {
    let lo = (0..=at.saturating_sub(100)).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0);
    let hi = (at + 140..=s.len()).find(|&i| s.is_char_boundary(i)).unwrap_or(s.len());
    s[lo..hi].to_string()
}

/// Entries of a package in archive order: (name, bytes).
fn entries(zip: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip)).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let mut d = Vec::new();
        f.read_to_end(&mut d).map_err(|e| e.to_string())?;
        out.push((f.name().to_string(), d));
    }
    Ok(out)
}

/// Every `<tag a="v" .../>` of a rels / content-types document as (tag, sorted attrs), sorted, so
/// the comparison ignores element order but nothing else. `None` if the text has a shape this
/// scanner does not handle (text content, CDATA); the caller then reports the plain XML class.
type Element = (String, Vec<(String, String)>);

fn element_set(xml: &str) -> Option<Vec<Element>> {
    let mut out = Vec::new();
    for chunk in xml.split('<').skip(1) {
        let t = chunk.trim_end();
        if t.starts_with('?') || t.starts_with('/') || t.starts_with('!') {
            continue;
        }
        let body = t.strip_suffix('>')?.trim_end_matches('/').trim_end();
        let (name, mut rest) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
        let mut attrs = Vec::new();
        loop {
            rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }
            let (k, v) = rest.split_once('=')?;
            let q = v.chars().next().filter(|c| *c == '"' || *c == '\'')?;
            let end = v[1..].find(q)? + 1;
            attrs.push((k.trim().to_string(), v[1..end].to_string()));
            rest = &v[end + 1..];
        }
        attrs.sort();
        out.push((name.to_string(), attrs));
    }
    out.sort();
    Some(out)
}

/// Effective content type of every non-rels part: Override by name, else Default by extension.
fn effective_types(entries: &[(String, Vec<u8>)]) -> BTreeMap<String, String> {
    let ct = entries.iter().find(|e| e.0 == "[Content_Types].xml").and_then(|e| element_set(&String::from_utf8_lossy(&e.1))).unwrap_or_default();
    let attr = |a: &[(String, String)], k: &str| a.iter().find(|x| x.0 == k).map(|x| x.1.clone());
    let (mut ov, mut def) = (BTreeMap::new(), BTreeMap::new());
    for (tag, a) in &ct {
        match (tag.as_str(), attr(a, "PartName"), attr(a, "Extension"), attr(a, "ContentType")) {
            ("Override", Some(n), _, Some(t)) => drop(ov.insert(n.trim_start_matches('/').to_ascii_lowercase(), t)),
            ("Default", _, Some(e), Some(t)) => drop(def.insert(e.to_ascii_lowercase(), t)),
            _ => {}
        }
    }
    entries
        .iter()
        .filter(|e| e.0 != "[Content_Types].xml")
        .map(|(n, _)| {
            let ext = n.rsplit_once('.').map_or("", |x| x.1).to_ascii_lowercase();
            (n.clone(), ov.get(&n.to_ascii_lowercase()).or_else(|| def.get(&ext)).cloned().unwrap_or_default())
        })
        .collect()
}

/// Sort the items of every kept-part `rels: [ChartRel..]` / `parts: [ChartPart..]` / `kept_parts: [KeptPart..]` list in a Debug
/// dump: they keep the order of the rels file, and the new writer sorts relationship elements by Id
/// as a string.
fn sort_rels_lists(s: &str) -> String {
    ["rels: [ChartRel", "parts: [ChartPart", "kept_parts: [KeptPart"].iter().fold(s.to_string(), |acc, m| sort_lists_after(&acc, m))
}

fn sort_lists_after(s: &str, marker: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while let Some(off) = s[i..].find(marker) {
        let open = i + off + marker.find('[').unwrap() + 1;
        out.push_str(&s[i..open]);
        let (mut depth, mut in_str, mut esc, mut j) = (1i32, false, false, open);
        let mut items: Vec<&str> = Vec::new();
        let mut start = open;
        while j < b.len() {
            let c = b[j];
            if in_str {
                if esc {
                    esc = false
                } else if c == b'\\' {
                    esc = true
                } else if c == b'"' {
                    in_str = false
                }
            } else {
                match c {
                    b'"' => in_str = true,
                    b'[' | b'{' | b'(' => depth += 1,
                    b']' | b'}' | b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    b',' if depth == 1 => {
                        items.push(s[start..j].trim());
                        start = j + 1;
                    }
                    _ => {}
                }
            }
            j += 1;
        }
        if start < j {
            items.push(s[start..j].trim());
        }
        items.sort_unstable();
        out.push_str(&items.join(", "));
        i = j;
    }
    out.push_str(&s[i..]);
    out
}

fn judge(old: &Run, new: &Run) -> Verdict {
    let mut classes: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut notes: Vec<String> = Vec::new();
    let mut hit = |c: &'static str, note: String| {
        *classes.entry(c).or_default() += 1;
        notes.push(format!("{c}: {note}"));
    };
    if old.skipped != new.skipped {
        hit("skipped", format!("old {:?} new {:?}", old.skipped, new.skipped));
    }
    if old.dump != new.dump {
        if sort_rels_lists(&old.dump) == sort_rels_lists(&new.dump) {
            hit("dump_rels_order", "kept-part relationship lists in a different order".into());
        } else {
            let at = first_diff(&old.dump, &new.dump);
            hit("dump", format!("@{at}\n  old ...{}\n  new ...{}", ctx(&old.dump, at), ctx(&new.dump, at)));
        }
    }
    match (&old.saved, &new.saved) {
        (Ok(o), Ok(n)) => match (entries(o), entries(n)) {
            (Ok(eo), Ok(en)) => {
                let (so, sn): (BTreeSet<&str>, BTreeSet<&str>) =
                    (eo.iter().map(|e| e.0.as_str()).collect(), en.iter().map(|e| e.0.as_str()).collect());
                if so != sn {
                    hit(
                        "part_set",
                        format!("only old {:?}; only new {:?}", so.difference(&sn).collect::<Vec<_>>(), sn.difference(&so).collect::<Vec<_>>()),
                    );
                } else if eo.iter().map(|e| &e.0).ne(en.iter().map(|e| &e.0)) {
                    hit("part_order", "same parts, different archive order".into());
                }
                let nm: BTreeMap<&str, &Vec<u8>> = en.iter().map(|e| (e.0.as_str(), &e.1)).collect();
                for (name, od) in &eo {
                    let Some(nd) = nm.get(name.as_str()) else { continue };
                    if od == *nd {
                        continue;
                    }
                    if name == "[Content_Types].xml"
                        && so == sn
                        && effective_types(&eo) == effective_types(&en)
                        && element_set(&String::from_utf8_lossy(od)) != element_set(&String::from_utf8_lossy(nd))
                    {
                        hit("ct_decl", "Default/Override declarations differ, effective type of every part equal".into());
                        continue;
                    }
                    if name.ends_with(".xml") || name.ends_with(".rels") || name.ends_with(".vml") {
                        let (a, b) = (String::from_utf8_lossy(od), String::from_utf8_lossy(nd));
                        match compare_xml(&a, &b) {
                            Ok(None) => {} // canonically equal
                            Ok(Some(path)) => {
                                let set_eq = (name.ends_with(".rels") || name == "[Content_Types].xml")
                                    && element_set(&a).is_some()
                                    && element_set(&a) == element_set(&b);
                                let class = match (set_eq, name.ends_with(".rels"), name == "[Content_Types].xml") {
                                    (true, true, _) => "rels_order",
                                    (true, _, true) => "ct_order",
                                    (_, true, _) => "rels_xml",
                                    (_, _, true) => "ct_xml",
                                    _ => "part_xml",
                                };
                                hit(class, format!("{name} at {path}"));
                            }
                            Err(e) => hit("part_unparsable", format!("{name}: {e:?}")),
                        }
                    } else {
                        hit("part_bytes", format!("{name}: {} vs {} bytes", od.len(), nd.len()));
                    }
                }
            }
            (a, b) => hit("save_unreadable", format!("old {:?} new {:?}", a.err(), b.err())),
        },
        (Err(a), Err(b)) => {
            if a != b {
                hit("save_err_text", format!("{a} / {b}"));
            }
        }
        (Ok(_), Err(e)) => hit("save_only_new_fails", e.clone()),
        (Err(e), Ok(_)) => hit("save_only_old_fails", e.clone()),
    }
    if old.reread != new.reread {
        match (&old.reread, &new.reread) {
            (Ok(a), Ok(b)) if sort_rels_lists(a) == sort_rels_lists(b) => {
                hit("reread_rels_order", "kept-part relationship lists in a different order".into())
            }
            (Ok(a), Ok(b)) => {
                let (a, b) = (sort_rels_lists(a), sort_rels_lists(b));
                let at = first_diff(&a, &b);
                hit("reread_dump", format!("@{at}\n  old ...{}\n  new ...{}", ctx(&a, at), ctx(&b, at)));
            }
            (a, b) => hit("reread_open", format!("old {:?} new {:?}", a.as_ref().err(), b.as_ref().err())),
        }
    }
    if classes.is_empty() {
        return Verdict::Identical;
    }
    DETAILS.lock().unwrap().insert(new.key, notes);
    // Classes that only describe how the package is laid out (entry order, relationship order,
    // Default-vs-Override declarations) share one metric so expected.toml can name them once.
    // Anything else is reported by its own class names; details.json keeps every class.
    const LAYOUT: [&str; 6] = ["part_order", "rels_order", "ct_order", "ct_decl", "dump_rels_order", "reread_rels_order"];
    let other: Vec<&str> = classes.keys().copied().filter(|c| !LAYOUT.contains(c)).collect();
    let metric = if other.is_empty() { "package_layout".to_string() } else { other.join("+") };
    Verdict::Exceeds { metric, value: classes.values().sum::<usize>() as f64, limit: 0.0 }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTS.contains(&e.to_ascii_lowercase().as_str())) {
            out.push(p);
        }
    }
}

/// Rewrite a package: `f` sees every (name, bytes) and returns the entries to emit in its place.
fn rewrite(src: &[u8], f: impl Fn(&str, Vec<u8>) -> Vec<(String, Vec<u8>)>, extra: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (n, d) in entries(src).unwrap() {
        for (n, d) in f(&n, d) {
            w.start_file(n, o).unwrap();
            w.write_all(&d).unwrap();
        }
    }
    for (n, d) in extra {
        w.start_file(n, o).unwrap();
        w.write_all(&d).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn replace_in(d: Vec<u8>, from: &str, to: &str) -> Vec<u8> {
    let s = String::from_utf8(d).unwrap();
    assert!(s.contains(from), "base package has no `{from}`");
    s.replacen(from, to, 1).into_bytes()
}

/// Small hand-made packages for the behaviour changes the corpus does not exercise. Built from
/// `export` of a default deck, so every case differs from `synthetic/control` in one thing.
fn synthetic() -> Vec<(String, Vec<u8>)> {
    let mut p = deckcraft_model::defaults::new_presentation(None);
    let id = p.slides[0].id;
    p.slide_mut(id).unwrap().notes = deckcraft_model::TextBody::from_text("notes");
    let base = deckcraft_pptx::export(&p).unwrap();
    let same = |n: &str, d: Vec<u8>| vec![(n.to_string(), d)];
    let on = |target: &'static str, edit: fn(Vec<u8>) -> Vec<u8>| {
        move |n: &str, d: Vec<u8>| if n == target { vec![(n.to_string(), edit(d))] } else { vec![(n.to_string(), d)] }
    };
    let slide_rels = "ppt/slides/_rels/slide1.xml.rels";
    let master_rels = "ppt/slideMasters/_rels/slideMaster1.xml.rels";
    let rel = |id: &str, kind: &str, target: &str| {
        format!("<Relationship Id=\"{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/{kind}\" Target=\"{target}\"/>")
    };
    let add_rel = |rels: &'static str, xml: String| {
        move |n: &str, d: Vec<u8>| {
            if n == rels {
                vec![(n.to_string(), String::from_utf8(d).unwrap().replacen("</Relationships>", &format!("{xml}</Relationships>"), 1).into_bytes())]
            } else {
                vec![(n.to_string(), d)]
            }
        }
    };
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut add = |name: &str, bytes: Vec<u8>| out.push((format!("synthetic/{name}.pptx"), bytes));
    add("control", rewrite(&base, same, vec![]));
    // duplicate relationship Id (second one points elsewhere)
    add("dup_rel_id", rewrite(&base, add_rel(slide_rels, rel("rId1", "hyperlink", "https://example.com/")), vec![]));
    // a relationship id that is a duplicate in an unrelated part's rels
    add("dup_rel_id_unused_part", rewrite(&base, add_rel("_rels/.rels", rel("rId1", "hyperlink", "https://example.com/")), vec![]));
    // percent-encoded targets that resolve to the existing parts
    add("pct_target_digit", rewrite(&base, on(master_rels, |d| replace_in(d, "slideLayout1.xml", "slideLayout%31.xml")), vec![]));
    add("pct_target_dotdot", rewrite(&base, add_rel(slide_rels, rel("rId99", "hyperlink", "%2e%2e/slideLayouts/slideLayout1.xml")), vec![]));
    add("pct_target_bad_escape", rewrite(&base, add_rel(slide_rels, rel("rId99", "hyperlink", "a%zzb.xml")), vec![]));
    // part whose name contains a space, referenced as %20
    add(
        "pct_target_space_part",
        rewrite(
            &base,
            |n, d| match n {
                "ppt/slideLayouts/slideLayout1.xml" => vec![("ppt/slideLayouts/slide Layout1.xml".to_string(), d)],
                "ppt/slideLayouts/_rels/slideLayout1.xml.rels" => vec![("ppt/slideLayouts/_rels/slide Layout1.xml.rels".to_string(), d)],
                "ppt/slideMasters/_rels/slideMaster1.xml.rels" => vec![(n.to_string(), replace_in(d, "slideLayout1.xml", "slide%20Layout1.xml"))],
                "[Content_Types].xml" => {
                    vec![(n.to_string(), replace_in(d, "/ppt/slideLayouts/slideLayout1.xml", "/ppt/slideLayouts/slide Layout1.xml"))]
                }
                _ => vec![(n.to_string(), d)],
            },
            vec![],
        ),
    );
    // backslash in an archive entry name, and in a relationship target
    add("backslash_entry_name", rewrite(&base, same, vec![("ppt\\media\\extra.bin".to_string(), b"x".to_vec())]));
    add(
        "backslash_entry_name_part",
        rewrite(
            &base,
            |n, d| {
                if n == "ppt/slideLayouts/slideLayout1.xml" {
                    vec![("ppt\\slideLayouts\\slideLayout1.xml".to_string(), d)]
                } else {
                    vec![(n.to_string(), d)]
                }
            },
            vec![],
        ),
    );
    add(
        "backslash_target",
        rewrite(&base, on(master_rels, |d| replace_in(d, "../slideLayouts/slideLayout1.xml", "..\\slideLayouts\\slideLayout1.xml")), vec![]),
    );
    // target that climbs out of the package
    add("target_escapes_package", rewrite(&base, add_rel(slide_rels, rel("rId99", "hyperlink", "../../../../x.xml")), vec![]));
    // fragment-only internal target (bookmark)
    add("target_fragment_only", rewrite(&base, add_rel(slide_rels, rel("rId99", "hyperlink", "#bookmark")), vec![]));
    // damaged archive entries nobody references
    // damaged archive entry nobody references: stored, one payload byte flipped, CRC now wrong
    add("bad_crc_unused_entry", {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        use std::io::Write;
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (n, d) in entries(&base).unwrap() {
            w.start_file(n, o).unwrap();
            w.write_all(&d).unwrap();
        }
        w.start_file("docProps/thumbnail.jpeg", o).unwrap();
        w.write_all(&[7u8; 64]).unwrap();
        let mut b = w.finish().unwrap().into_inner();
        // flip one payload byte of the thumbnail (64 x 0x07 run) so its stored CRC no longer matches
        let at = b.windows(64).position(|x| x.iter().all(|c| *c == 7)).unwrap();
        b[at + 5] ^= 0xff;
        b
    });
    // two parts whose names differ only in case
    add("case_duplicate_part", rewrite(&base, same, vec![("PPT/presProps.xml".to_string(), b"<x/>".to_vec())]));
    // unrelated relationships part that is not XML
    add(
        "broken_rels_xml_unused_part",
        rewrite(&base, on("ppt/slideLayouts/_rels/slideLayout1.xml.rels", |_| b"<Relationships><Relationship".to_vec()), vec![]),
    );
    // one part larger than the 512 MiB per-part cap (compresses to ~0.5 MB)
    add("oversized_part", rewrite(&base, same, vec![("ppt/media/huge.bin".to_string(), vec![0u8; 513 << 20])]));
    out
}

#[test]
#[ignore = "needs the Office corpus: set DECKCRAFT_OFFICE_CORPUS"]
fn ooxml_old_vs_new() {
    let Ok(root) = std::env::var("DECKCRAFT_OFFICE_CORPUS") else {
        eprintln!("DECKCRAFT_OFFICE_CORPUS not set; skipping");
        return;
    };
    let root = PathBuf::from(root);
    let mut files = Vec::new();
    let mut keys = BTreeMap::new();
    for c in CORPORA {
        let mut paths = Vec::new();
        walk(&root.join(c), &mut paths);
        for p in paths {
            let bytes = std::fs::read(&p).unwrap();
            let name = format!("{c}/{}", p.strip_prefix(root.join(c)).unwrap().display());
            keys.insert(fnv(&bytes), name.clone());
            files.push((name, bytes));
        }
    }
    for (name, bytes) in synthetic() {
        keys.insert(fnv(&bytes), name.clone());
        files.push((name, bytes));
    }
    eprintln!("{} files", files.len());
    let registry = Registry::from_path(&Path::new(env!("CARGO_MANIFEST_DIR")).join("expected.toml")).unwrap();
    let out = PathBuf::from(std::env::var("DECKCRAFT_ORACLE_OUT").unwrap_or_else(|_| "target/oracle".into()));
    let report = Differential::new(FnSide(side!(pptx_old)), FnSide(side!(deckcraft_pptx)), FnJudge(judge))
        .timeout(Duration::from_secs(300))
        .expected(registry)
        .artifact_dir(out.join("failures"))
        .run_files(files);
    let policy = Policy::default();
    report.write(&out, &policy).unwrap();

    // per-corpus counts
    let mut per: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut summary = String::new();
    for r in &report.results {
        let c = CORPORA.iter().chain(["synthetic"].iter()).find(|c| r.name.starts_with(&format!("{c}/"))).unwrap();
        *per.entry(c).or_default().entry(r.outcome.name().to_string()).or_default() += 1;
        if let Outcome::ExpectedDifference { id } = &r.outcome {
            *ids.entry(id.clone()).or_default() += 1;
        }
    }
    summary += &format!("total {}\n\nper corpus:\n", report.total);
    for (c, m) in &per {
        summary += &format!("  {c}: {m:?}\n");
    }
    summary += &format!("\nexpected-difference ids: {ids:?}\nstale: {:?}\n", report.stale);
    let mut metrics: BTreeMap<String, usize> = BTreeMap::new();
    let mut both: BTreeMap<String, usize> = BTreeMap::new();
    let mut onesided: BTreeMap<String, usize> = BTreeMap::new();
    for r in &report.results {
        match &r.outcome {
            Outcome::Regression { metric, .. } => *metrics.entry(metric.clone()).or_default() += 1,
            Outcome::BothFail { old_err, new_err } => *both.entry(format!("old={old_err} | new={new_err}")).or_default() += 1,
            Outcome::OnlyOldFails { old_err } => *onesided.entry(format!("OnlyOld: {old_err}")).or_default() += 1,
            Outcome::OnlyNewFails { new_err } => *onesided.entry(format!("OnlyNew: {new_err}")).or_default() += 1,
            _ => {}
        }
    }
    summary += &format!("\nregression metrics: {metrics:#?}\nboth-fail classes: {both:#?}\none-sided: {onesided:#?}\n");
    let mut class_counts: BTreeMap<String, usize> = BTreeMap::new();
    for notes in DETAILS.lock().unwrap().values() {
        for c in notes.iter().map(|n| n.split(':').next().unwrap_or("")).collect::<BTreeSet<_>>() {
            *class_counts.entry(c.to_string()).or_default() += 1;
        }
    }
    summary += &format!("\nfiles showing each difference class (a file can show several): {class_counts:#?}\n");
    std::fs::write(out.join("summary.md"), &summary).unwrap();
    let details: BTreeMap<String, Vec<String>> =
        DETAILS.lock().unwrap().iter().filter_map(|(k, v)| keys.get(k).map(|n| (n.clone(), v.clone()))).collect();
    std::fs::write(out.join("details.json"), serde_json::to_string_pretty(&details).unwrap()).unwrap();
    eprintln!("{summary}");
    assert_eq!(report.exit_code(&policy), 0, "unexplained differences; see {}", out.display());
}

/// Debug aid: `DECKCRAFT_INSPECT=<file>` writes `old.pptx` / `new.pptx` (open -> save of each
/// side) into `$DECKCRAFT_ORACLE_OUT` so the two packages can be unzipped and diffed.
#[test]
#[ignore = "debug aid: set DECKCRAFT_INSPECT"]
fn inspect_one() {
    let Ok(f) = std::env::var("DECKCRAFT_INSPECT") else { return };
    let out = PathBuf::from(std::env::var("DECKCRAFT_ORACLE_OUT").unwrap_or_else(|_| "target/oracle".into()));
    let bytes = std::fs::read(f).unwrap();
    for (n, r) in [("old", side!(pptx_old)(&bytes)), ("new", side!(deckcraft_pptx)(&bytes))] {
        match r {
            Ok(r) => std::fs::write(out.join(format!("{n}.pptx")), r.saved.unwrap()).unwrap(),
            Err(e) => eprintln!("{n}: open failed: {e}"),
        }
    }
}
