//! Open Packaging Conventions: the zip container, part names, relationships and content types.

use craft_ooxml::opc::{OpcLimits, Package as OpcPackage, PackageWriter, TargetMode, WritePolicy, resolve_target};

use crate::PptxError;
use crate::xml::{self, LIMITS, fn1};

pub const MAX_XML_PART: usize = 96 * 1024 * 1024;
pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
pub const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
pub const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
pub const NS_P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
pub const NS_P15: &str = "http://schemas.microsoft.com/office/powerpoint/2012/main";
pub const NS_P159: &str = "http://schemas.microsoft.com/office/powerpoint/2015/09/main";
pub const NS_A14: &str = "http://schemas.microsoft.com/office/drawing/2010/main";
pub const NS_P188: &str = "http://schemas.microsoft.com/office/powerpoint/2018/8/main";
pub const RT_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
pub const RT_MEDIA: &str = "http://schemas.microsoft.com/office/2007/relationships/media";
pub const RT_MODEL3D: &str = "http://schemas.microsoft.com/office/2017/06/relationships/model3d";
pub const RT_CORE: &str = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";

pub struct Package(OpcPackage);
impl Package {
    pub fn open(bytes: &[u8]) -> Result<Self, PptxError> {
        let mut lim = OpcLimits { xml: LIMITS, max_archive_bytes: 2 << 30, ..OpcLimits::default() };
        (lim.zip.max_entries, lim.zip.max_entry_bytes, lim.zip.max_total_bytes) = (50_000, 512 << 20, 2 << 30);
        OpcPackage::open(bytes, &lim).map(Self).map_err(|e| PptxError::NotPptx(e.to_string()))
    }
    fn1! {
        pub fn get(&self, name: &str) -> Option<&[u8]> { self.0.part(&norm(name)) }
        pub fn xml_bytes(&self, name: &str) -> Option<&[u8]> { self.get(name).filter(|b| b.len() <= MAX_XML_PART) }
        pub fn xml(&self, name: &str) -> Option<xml::Doc> { xml::parse(self.xml_bytes(name)?).ok() }
        pub fn rels(&self, part: &str) -> Rels { self.xml_bytes(&rels_path(part)).map_or_else(Rels::default, |b| rels_from_bytes(b, part)) }
        pub fn content_type(&self, part: &str) -> Option<String> { self.0.content_types.of(&norm(part)).map(str::to_string) }
    }
}

pub fn rels_from_bytes(bytes: &[u8], part: &str) -> Rels {
    let (Ok(doc), base, mut items) = (xml::parse(bytes), dir_of(part), Vec::new()) else { return Rels::default() };
    for r in doc.root.children_named("Relationship") {
        let (Some(id), Some(ty), Some(raw)) = (r.attr("Id"), r.attr("Type"), r.attr("Target")) else { continue };
        let external = r.attr("TargetMode").is_some_and(|m| m.eq_ignore_ascii_case("external"));
        let (target_raw, target) = (raw.to_string(), if external { raw.into() } else { resolve(&base, raw) });
        items.push(Rel { id: id.into(), kind: ty.rsplit('/').next().unwrap_or(ty).into(), ty: ty.into(), target_raw, target, external });
    }
    Rels { items }
}

#[derive(Clone, Debug, Default)]
pub struct Rel {
    pub id: String,
    pub kind: String,
    pub ty: String,
    pub target_raw: String,
    pub target: String,
    pub external: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Rels {
    pub items: Vec<Rel>,
}
impl Rels {
    fn1! {
        pub fn get(&self, id: &str) -> Option<&Rel> { self.items.iter().find(|r| r.id == id) }
        pub fn target(&self, id: &str) -> Option<&str> { self.get(id).map(|r| r.target.as_str()) }
        pub fn first_of(&self, kind: &str) -> Option<&Rel> { self.items.iter().find(|r| r.kind == kind) }
    }
    pub fn all_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Rel> + 'a {
        self.items.iter().filter(move |r| r.kind == kind)
    }
}

fn1! {
    pub fn norm(name: &str) -> String { name.trim_start_matches('/').replace('\\', "/").to_ascii_lowercase() }
    pub fn dir_of(part: &str) -> String { part.trim_start_matches('/').rsplit_once('/').map_or_else(String::new, |(d, _)| d.into()) }
    pub fn file_of(part: &str) -> &str { part.rsplit('/').next().unwrap_or(part) }
    pub fn rels_path(part: &str) -> String {
        let (dir, file) = (dir_of(part), file_of(part.trim_start_matches('/')));
        if dir.is_empty() { format!("_rels/{file}.rels") } else { format!("{dir}/_rels/{file}.rels") }
    }
    pub fn resolve(base: &str, target: &str) -> String {
        let src = if base.trim_matches('/').is_empty() { String::new() } else { format!("{}/_", base.trim_matches('/')) };
        let target = target.replace('\\', "/");
        // A fragment-only target ("#bookmark") names no part. Before the swap it resolved to the
        // base directory; `resolve_target` would return the synthetic source `<base>/_` instead.
        if target.starts_with('#') {
            return base.split('/').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("/");
        }
        resolve_target(&src, &target).unwrap_or_default()
    }
}
pub fn relative(from: &str, to: &str) -> String {
    let (fd_s, td_s) = (dir_of(from), dir_of(to));
    let (fd, td): (Vec<_>, Vec<_>) = (fd_s.split('/').filter(|s| !s.is_empty()).collect(), td_s.split('/').filter(|s| !s.is_empty()).collect());
    let common = fd.iter().zip(&td).take_while(|(a, b)| a == b).count();
    format!("{}{}{}", "../".repeat(fd.len() - common), td[common..].iter().map(|s| format!("{s}/")).collect::<String>(), file_of(to))
}
pub fn sidecar_part(name: &str) -> bool {
    let n = norm(name);
    let s: Vec<_> = n.split('/').filter(|s| !s.is_empty()).collect();
    let num = s
        .last()
        .copied()
        .unwrap_or("")
        .strip_prefix("chart")
        .and_then(|x| x.strip_suffix(".xml.rels").or_else(|| x.strip_suffix(".xml")))
        .unwrap_or("");
    matches!((s.first(), s.get(1)), (Some(&"ppt"), Some(&("embeddings" | "charts" | "drawings" | "media"))))
        && !s.iter().any(|x| *x == "." || *x == "..")
        && (num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()))
}

#[derive(Default)]
pub struct RelsOut {
    pub items: Vec<(String, String, String, bool)>,
}
impl RelsOut {
    fn1! {
        pub fn add(&mut self, kind: &str, target: &str, ext: bool) -> String {
            let ty = if kind.starts_with("http") { kind.into() } else { format!("{RT_BASE}{kind}") };
            self.items.iter().find(|(_, t, tg, e)| *t == ty && tg == target && *e == ext).map(|(id, ..)| id.clone()).unwrap_or_else(|| self.add_new(kind, target, ext))
        }
        pub fn add_new(&mut self, kind: &str, target: &str, ext: bool) -> String {
            let id = format!("rId{}", self.items.len() + 1);
            (self.items.push((id.clone(), if kind.starts_with("http") { kind.into() } else { format!("{RT_BASE}{kind}") }, target.into(), ext)), id).1
        }
    }
}

pub struct PackageOut(PackageWriter);
impl Default for PackageOut {
    fn default() -> Self {
        Self(PackageWriter::with_policy(WritePolicy::deckcraft()))
    }
}
impl PackageOut {
    fn1! {
        pub fn xml_part(&mut self, name: &str, ct: &str, data: Vec<u8>) { let _ = self.0.add_part(name, ct, data); }
        pub fn binary(&mut self, name: &str, ct: &str, data: Vec<u8>) { let _ = self.0.add_part(name, ct, data); }
        pub fn finish(self) -> Result<Vec<u8>, PptxError> { self.0.finish().map_err(|e| PptxError::Write(e.to_string())) }
    }
    pub fn rels(&mut self, part: &str, rels: &RelsOut) {
        for (id, ty, target, ext) in &rels.items {
            let _ = self.0.add_relationship(part, id, ty, target, if *ext { TargetMode::External } else { TargetMode::Internal });
        }
    }
    pub fn raw(&mut self, name: &str, data: Vec<u8>) {
        if let Some((dir, file)) = name.rsplit_once("/_rels/").or_else(|| name.strip_prefix("_rels/").map(|f| ("", f)))
            && let Some(base) = file.strip_suffix(".rels")
        {
            // A kept part's sidecar is carried over byte-for-byte; only if the shared writer
            // rejects it (unsafe target, already-added source) fall back to re-adding its rows.
            let src = if dir.is_empty() { base.to_string() } else { format!("{dir}/{base}") };
            if self.0.add_relationships_bytes(&src, data.clone()).is_ok() {
                return;
            }
            for r in rels_from_bytes(&data, &src).items {
                let mode = if r.external { TargetMode::External } else { TargetMode::Internal };
                let _ = self.0.add_relationship(&src, &r.id, &r.ty, &r.target_raw, mode);
            }
        }
    }
}
