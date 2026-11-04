//! PPTX import and export: Office Open XML PresentationML / DrawingML, written from the public
//! ECMA-376 standard.
//!
//! [`import`] reads a package leniently (unknown parts are skipped, malformed parts are logged and
//! ignored, every size and depth is bounded) into the DeckCraft model; [`export`] writes a valid
//! package that PowerPoint opens without repair.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod cfb;
mod custgeom;
mod dgm;
mod maps;
mod opc;
mod png;
pub(crate) mod read;
pub use read::MAX_TAG_BYTES;
mod tables;
mod write;
mod xml;

/// The bounded XML reader the importer uses, for callers that read kept fragments (a shape's
/// 3-D settings) instead of scanning strings.
pub use xml::{Doc as XmlDoc, El as XmlEl, parse as parse_xml};

use deckcraft_model::Presentation;

/// Extension URI of the PowerPoint 2010 section list in `presentation.xml`.
pub(crate) const SECTION_EXT_URI: &str = "{521415D9-36F7-43E2-AB2F-B90AF26B5E84}";

/// Extension URI of the modern-comments anchor in a slide (names the threaded comment part).
pub(crate) const COMMENTS_EXT_URI: &str = "{6950BFC3-D8DA-4A85-94F7-54DA5524770B}";

#[derive(Debug, thiserror::Error)]
pub enum PptxError {
    #[error("not a PPTX file: {0}")]
    NotPptx(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("write failed: {0}")]
    Write(String),
    #[error("malformed XML: {0}")]
    Xml(String),
}

/// Does this look like an OOXML package (zip with `[Content_Types].xml`)?
///
/// The part is looked up in the zip's central directory, not in the leading bytes: writers may
/// store it anywhere in the archive (recent PowerPoint builds put it last, megabytes in).
pub fn sniff(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"PK") {
        return false;
    }
    zip::ZipArchive::new(std::io::Cursor::new(bytes)).map(|z| z.index_for_name("[Content_Types].xml").is_some()).unwrap_or(false)
}

/// Is this a password-protected presentation (an encrypted package inside a compound file)?
pub fn is_encrypted(bytes: &[u8]) -> bool {
    cfb::is_encrypted(bytes)
}

/// Read a `.pptx` / `.potx` / `.ppsx` package.
pub fn import(bytes: &[u8]) -> Result<Presentation, PptxError> {
    read::import(bytes)
}

/// [`import`], plus the part names of slides the file lists but that couldn't be read and were
/// left out.
pub fn import_checked(bytes: &[u8]) -> Result<(Presentation, Vec<String>), PptxError> {
    read::import_checked(bytes)
}

/// Read the theme of a `.thmx` file, or of a `.pptx` / `.potx`'s first master, with its media.
pub fn import_theme(bytes: &[u8]) -> Result<(deckcraft_model::theme::Theme, Vec<deckcraft_model::MediaItem>), PptxError> {
    read::import_theme(bytes)
}

/// Write a `.pptx` package.
pub fn export(p: &Presentation) -> Result<Vec<u8>, PptxError> {
    write::export(p, write::CT_PRES)
}

/// Write a package for `ext`: `potx` (template) and `ppsx` (slide show) differ from `pptx` only in
/// the main part's content type.
pub fn export_as(p: &Presentation, ext: &str) -> Result<Vec<u8>, PptxError> {
    let ct = match ext {
        "potx" => write::CT_TEMPLATE,
        "ppsx" => write::CT_SHOW,
        _ => write::CT_PRES,
    };
    write::export(p, ct)
}

/// Content type for a file name's extension.
pub(crate) fn mime_for(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "svg" => "image/svg+xml",
        "emf" => "image/x-emf",
        "wmf" => "image/x-wmf",
        "webp" => "image/webp",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "wmv" => "video/x-ms-wmv",
        "avi" => "video/x-msvideo",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "wma" => "audio/x-ms-wma",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "glb" => "model/gltf-binary",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "fntdata" => "application/x-fontdata",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod import_theme_tests {
    use std::io::{Read, Write};

    use deckcraft_model::Presentation;

    fn custom() -> Presentation {
        let mut p = Presentation::default();
        let m = std::sync::Arc::make_mut(&mut p.masters[0]);
        m.theme.name = "Harbour".into();
        m.theme.fonts.major.latin = "Lato".into();
        m.theme.fonts.minor.latin = "Source Sans 3".into();
        m.theme.colors.set(deckcraft_color::SchemeSlot::Accent1, deckcraft_color::Rgba::rgb(0x12, 0x34, 0x56));
        p
    }

    /// A `.thmx`: the deck's theme part as the package's main document.
    fn thmx(pptx: &[u8]) -> Vec<u8> {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(pptx)).unwrap();
        let mut theme = String::new();
        z.by_name("ppt/theme/theme1.xml").unwrap().read_to_string(&mut theme).unwrap();
        let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        out.start_file("[Content_Types].xml", o).unwrap();
        out.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/theme/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>"#).unwrap();
        out.start_file("_rels/.rels", o).unwrap();
        out.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="theme/theme/theme1.xml"/></Relationships>"#).unwrap();
        out.start_file("theme/theme/theme1.xml", o).unwrap();
        out.write_all(theme.as_bytes()).unwrap();
        out.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_the_theme_of_a_pptx_and_of_a_thmx() {
        let pptx = crate::export(&custom()).unwrap();
        for bytes in [pptx.clone(), thmx(&pptx)] {
            let (t, _) = crate::import_theme(&bytes).unwrap();
            assert_eq!(t.name, "Harbour");
            assert_eq!(t.fonts.major.latin, "Lato");
            assert_eq!(t.fonts.minor.latin, "Source Sans 3");
            assert_eq!(t.colors.get(deckcraft_color::SchemeSlot::Accent1), deckcraft_color::Rgba::rgb(0x12, 0x34, 0x56));
        }
    }

    #[test]
    fn non_theme_packages_are_errors() {
        assert!(crate::import_theme(b"not a zip").is_err());
        let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        out.start_file("[Content_Types].xml", zip::write::SimpleFileOptions::default()).unwrap();
        out.write_all(b"<Types/>").unwrap();
        assert!(crate::import_theme(&out.finish().unwrap().into_inner()).is_err());
    }
}

#[cfg(test)]
mod xml_opc_tests {
    use crate::opc::*;
    use crate::xml::*;
    use std::fmt::Write as _;
    use std::io::{Cursor, Read, Write};

    struct El(String, Vec<(String, String)>, Vec<Result<El, String>>);
    fn1! {
        fn esc(s: &str, a: bool) -> String {
            let ok = |c: char| matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}');
            s.chars().fold(String::new(), |mut o, c| {
                let r = if c == '&' { "&amp;" } else if c == '<' { "&lt;" } else if c == '>' { "&gt;" } else if c == '\r' { "&#13;" } else if a && c == '"' { "&quot;" } else if a && c == '\n' { "&#10;" } else if a && c == '\t' { "&#9;" } else { if ok(c) { o.push(c); } return o; };
                (o.push_str(r), o).1
            })
        }
        fn write_el(e: &El, s: &mut String, d: usize) {
            if d > MAX_DEPTH { return; }
            let _ = write!(s, "<{}", e.0);
            for (k, v) in &e.1 { let _ = write!(s, " {}=\"{}\"", k, esc(v, true)); }
            if e.2.is_empty() { return s.push_str("/>"); }
            s.push('>');
            for c in &e.2 { if let Ok(c) = c { write_el(c, s, d + 1); } else if let Err(t) = c { s.push_str(&esc(t, false)); } }
            let _ = write!(s, "</{}>", e.0);
        }
        fn old_parse(bytes: &[u8]) -> Option<El> {
            use quick_xml::events::Event as E;
            let mut r = quick_xml::Reader::from_reader(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes));
            r.config_mut().trim_text(false); r.config_mut().check_end_names = false; r.config_mut().allow_unmatched_ends = true;
            let (mut st, mut root, mut buf) = (Vec::<El>::new(), None, Vec::new());
            let push_t = |p: &mut El, s: &str| if !s.is_empty() { if let Some(Err(t)) = p.2.last_mut() { t.push_str(s); } else { p.2.push(Err(s.into())); } };
            let start = |b: &quick_xml::events::BytesStart, r: &quick_xml::Reader<&[u8]>| El(String::from_utf8_lossy(b.name().as_ref()).into(), b.attributes().with_checks(false).flatten().map(|a| (String::from_utf8_lossy(a.key.as_ref()).into(), a.decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, r.decoder()).map(|c| c.into_owned()).unwrap_or_default())).collect(), vec![]);
            loop {
                match r.read_event_into(&mut buf).ok()? {
                    E::Start(b) if root.is_none() => st.push(start(&b, &r)),
                    E::Empty(b) if root.is_none() => { let el = start(&b, &r); if let Some(p) = st.last_mut() { p.2.push(Ok(el)); } else { root = Some(el); } }
                    E::End(_) => if let Some(el) = st.pop() { if let Some(p) = st.last_mut() { p.2.push(Ok(el)); } else { root = Some(el); } },
                    E::Text(t) => if let Some(p) = st.last_mut() { push_t(p, &t.decode().unwrap_or_default().replace("\r\n", "\n").replace('\r', "\n")); },
                    E::CData(t) => if let Some(p) = st.last_mut() { push_t(p, &String::from_utf8_lossy(&t).replace("\r\n", "\n").replace('\r', "\n")); },
                    E::GeneralRef(g) => if let Some(p) = st.last_mut() { push_t(p, &if g.is_char_ref() { g.resolve_char_ref().ok().flatten().map(String::from).unwrap_or_default() } else { [("amp", "&"), ("lt", "<"), ("gt", ">"), ("quot", "\""), ("apos", "'")].iter().find(|(k, _)| *k == g.decode().unwrap_or_default()).map_or("", |(_, v)| v).into() }); },
                    E::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
            root
        }
        fn pkg_finish(files: &[(String, Vec<u8>, bool)], overrides: &[(String, String)]) -> Vec<u8> {
            let mut ct = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>");
            for (p, c) in overrides { let _ = write!(ct, "<Override PartName=\"{}\" ContentType=\"{}\"/>", esc(p, true), esc(c, true)); }
            ct.push_str("</Types>");
            let (mut zw, def, sto) = (zip::ZipWriter::new(Cursor::new(Vec::new())), zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated), zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored));
            zw.start_file("[Content_Types].xml", def).unwrap(); zw.write_all(ct.as_bytes()).unwrap();
            for (n, d, c) in files { zw.start_file(n.as_str(), if *c { def } else { sto }).unwrap(); zw.write_all(d).unwrap(); }
            zw.finish().unwrap().into_inner()
        }
        fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
            let (mut zw, o) = (zip::ZipWriter::new(Cursor::new(Vec::new())), zip::write::SimpleFileOptions::default());
            zw.start_file("[Content_Types].xml", o).unwrap(); zw.write_all(b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"/>").unwrap();
            for (n, d) in entries { zw.start_file(*n, o).unwrap(); zw.write_all(d).unwrap(); }
            zw.finish().unwrap().into_inner()
        }
    }

    macro_rules! t { ($(fn $n:ident() $b:block)*) => { $(#[test] fn $n() $b)* }; }
    t! {
        fn oracle_xml_and_package_identity() {
            let mut p = deckcraft_model::defaults::new_presentation(None);
            let id = p.slides[0].id;
            p.slide_mut(id).unwrap().notes = deckcraft_model::TextBody::from_text("Notes & <quote> \"here\"");
            let exported = crate::export(&p).unwrap();
            let (pkg, mut z, mut files, mut ov, mut new_pkg) = (Package::open(&exported).unwrap(), zip::ZipArchive::new(Cursor::new(&exported)).unwrap(), Vec::new(), Vec::new(), PackageOut::default());
            for i in 0..z.len() {
                let (mut f, mut data) = (z.by_index(i).unwrap(), Vec::new());
                let name = f.name().to_string();
                f.read_to_end(&mut data).unwrap();
                if name.ends_with(".xml") || name.ends_with(".rels") {
                    let mut old_xml = String::new();
                    write_el(&old_parse(&data).unwrap(), &mut old_xml, 0);
                    assert_eq!(old_xml, parse(&data).unwrap().root.to_xml(), "XML mismatch on {name}");
                }
                if name != "[Content_Types].xml" {
                    if name.ends_with(".rels") { files.push((name.clone(), data.clone(), true)); new_pkg.raw(&name, data); }
                    else if let Some(ct) = pkg.content_type(&name) { ov.push((format!("/{name}"), ct.clone())); files.push((name.clone(), data.clone(), true)); new_pkg.xml_part(&name, &ct, data); }
                }
            }
            let (mut oz, mut nz) = (zip::ZipArchive::new(Cursor::new(pkg_finish(&files, &ov))).unwrap(), zip::ZipArchive::new(Cursor::new(new_pkg.finish().unwrap())).unwrap());
            assert_eq!(oz.len(), nz.len());
            for i in 0..oz.len() {
                let (of, nf) = (oz.by_index(i).unwrap(), nz.by_index(i).unwrap());
                assert_eq!((of.name(), of.size(), of.crc32(), of.compression()), (nf.name(), nf.size(), nf.crc32(), nf.compression()));
            }
        }
        fn literal_cr() { assert_eq!(parse(b"<a:p><a:t>one\r\ntwo\rthree&#13;four</a:t><a:t><![CDATA[x\r\ny]]></a:t></a:p>").unwrap().root.children_named("t").map(|t| t.text()).collect::<Vec<_>>(), vec!["one\ntwo\nthree\rfour", "x\ny"]); }
        fn cr_survives() { let mut w = W::default(); w.elt("a:t", "one\rtwo\r\nthree"); assert_eq!(parse(w.s.as_bytes()).unwrap().root.text(), "one\rtwo\r\nthree"); }
        fn entities() { let d = parse(b"<?xml version=\"1.0\"?><a:p xmlns:a=\"x\"><a:r><a:t>A &amp; B &#x41; &lt;</a:t></a:r><a:x r:id=\"rId1\" id=\"5\"/></a:p>").unwrap(); assert_eq!((d.root.name.as_str(), d.root.find("t").unwrap().text().as_str(), d.root.child("x").unwrap().attr("id"), d.root.child("x").unwrap().attr_ns("r", "id"), d.ns_decls().len()), ("p", "A & B A <", Some("5"), Some("rId1"), 1)); }
        fn truncated() { assert!(parse(b"<a><b><c>text").is_ok() && parse(b"").is_err() && parse("<a>".repeat(MAX_DEPTH + 10).as_bytes()).is_err()); }
        fn escapes() { assert_eq!((craft_ooxml::write::escape_text("a\u{b}b<\u{0}"), craft_ooxml::write::escape_attr("\"x\"\n")), ("ab&lt;".into(), "&quot;x&quot;&#10;".into())); }
        fn roundtrip_ser() { assert_eq!(parse(b"<p:sp a=\"1\"><p:t>x &amp; y</p:t><e/></p:sp>").unwrap().root.to_xml(), "<p:sp a=\"1\"><p:t>x &amp; y</p:t><e/></p:sp>"); }
        fn resolve_hash() { assert_eq!((resolve("ppt/slides", "../media/a%23b.png"), resolve("ppt/slides", "../media/a.png#frag")), ("ppt/media/a#b.png".into(), "ppt/media/a.png".into())); }
        fn resolve_fragment_only() { assert_eq!((resolve("ppt/slides", "#_ftn1"), resolve("ppt/slides/", "#"), resolve("", "#x")), ("ppt/slides".into(), "ppt/slides".into(), String::new())); }
        fn resolve_pct() { assert_eq!(resolve("ppt/slides", "../media/a%20b.png"), "ppt/media/a b.png"); }
        fn xml_limit() { let p = Package::open(&zip_of(&[("small.xml", &[b'x'; 10])])).unwrap(); assert_eq!((p.xml_bytes("small.xml").map(<[u8]>::len), p.xml_bytes("missing.xml")), (Some(10), None)); }
        fn resolve_bs() { assert_eq!((resolve("ppt/slides", "..\\media\\image1.png"), resolve("ppt/slides", "\\ppt\\media\\a.png")), ("ppt/media/image1.png".into(), "ppt/media/a.png".into())); }
        fn paths() { assert_eq!((resolve("ppt/slides", "../media/image1.png"), resolve("ppt", "slides/slide1.xml"), rels_path("ppt/presentation.xml"), relative("ppt/slides/slide1.xml", "ppt/media/image1.png")), ("ppt/media/image1.png".into(), "ppt/slides/slide1.xml".into(), "ppt/_rels/presentation.xml.rels".into(), "../media/image1.png".into())); }
        fn sidecars() { assert!(sidecar_part("ppt/embeddings/a.xlsx") && sidecar_part("ppt/charts/style1.xml") && !sidecar_part("ppt/charts/chart1.xml") && !sidecar_part("ppt/slides/slide1.xml") && !sidecar_part("/etc/passwd")); }
        fn rel_targets() { assert_eq!((relative("ppt/presentation.xml", "ppt/slides/slide1.xml"), relative("ppt/slides/slide1.xml", "ppt/slides/slide2.xml")), ("slides/slide1.xml".into(), "slide2.xml".into())); }
        fn rel_root() { assert_eq!(rels_path("[Content_Types].xml"), "_rels/[Content_Types].xml.rels"); }
        fn writer_attrs() { assert_eq!(String::from_utf8( { let mut w = W::frag(); w.empty("a:b", A::new().o("x", Some(1)).b("y", Some(false)).t("z", true)); w.finish() } ).unwrap(), "<a:b x=\"1\" y=\"0\" z=\"1\"/>"); }
        fn pkg_rels_out() { let mut r = RelsOut::default(); assert_eq!(r.add("slide", "slides/slide1.xml", false), r.add("slide", "slides/slide1.xml", false)); }
    }
}
