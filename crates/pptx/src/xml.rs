//! Bounded XML DOM and writer backed by `craft-ooxml`.

pub use craft_ooxml::dom::{El, Limits, Namespaces, Node, ReadMode};
use craft_ooxml::write::{XmlWriter, escape_attr, escape_text};

use crate::PptxError;

pub const MAX_DEPTH: usize = 256;
pub const MAX_NODES: usize = 4_000_000;
pub const LIMITS: Limits = Limits {
    max_depth: MAX_DEPTH,
    max_nodes: MAX_NODES,
    max_text_bytes: crate::opc::MAX_XML_PART,
    max_attr_bytes: crate::opc::MAX_XML_PART,
    max_total_bytes: crate::opc::MAX_XML_PART,
    read_mode: ReadMode::Lenient,
    namespaces: Namespaces::Prefix,
};

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub root: El,
}
impl Doc {
    pub fn ns_decls(&self) -> Vec<(String, String)> {
        self.root.attrs.iter().filter(|(k, _)| k == "xmlns" || k.starts_with("xmlns:")).cloned().collect()
    }
}

pub fn parse(bytes: &[u8]) -> Result<Doc, PptxError> {
    craft_ooxml::dom::parse(bytes, &LIMITS).map(|root| Doc { root }).map_err(|e| PptxError::Xml(e.to_string()))
}

#[derive(Default)]
pub struct W {
    pub s: String,
}
#[derive(Default)]
pub struct A {
    pub v: Vec<(&'static str, String)>,
}

macro_rules! fn1 {
    ($($vis:vis fn $n:ident $(<$t:ident : $b:ident>)? ($($a:tt)*) $(-> $r:ty)? $body:block)*) => { $($vis fn $n $(<$t: $b>)? ($($a)*) $(-> $r)? $body)* };
}
pub(crate) use fn1;

impl A {
    fn1! {
        pub fn new() -> Self { Self::default() }
        pub fn a(mut self, k: &'static str, v: impl ToString) -> Self { (self.v.push((k, v.to_string())), self).1 }
        pub fn o<T: ToString>(self, k: &'static str, v: Option<T>) -> Self { if let Some(v) = v { self.a(k, v) } else { self } }
        pub fn b(self, k: &'static str, v: Option<bool>) -> Self { if let Some(v) = v { self.a(k, if v { "1" } else { "0" }) } else { self } }
        pub fn t(self, k: &'static str, v: bool) -> Self { if v { self.a(k, "1") } else { self } }
    }
    fn emit(self, name: &str, close: &str, out: &mut String) {
        let mut w = XmlWriter::deckcraft();
        w.start(name);
        for (k, v) in self.v {
            w.attr(k, &v);
        }
        let _ = (if close == "/>" { w.empty() } else { w.text("") }, escape_attr(""));
        out.push_str(std::str::from_utf8(&w.finish()).unwrap_or_default());
    }
}

impl W {
    fn1! {
        pub fn new() -> Self { Self { s: "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n".into() } }
        pub fn frag() -> Self { Self::default() }
        pub fn open(&mut self, name: &str, a: A) { a.emit(name, ">", &mut self.s); }
        pub fn open0(&mut self, name: &str) { self.s.extend(["<", name, ">"]); }
        pub fn close(&mut self, name: &str) { self.s.extend(["</", name, ">"]); }
        pub fn empty(&mut self, name: &str, a: A) { a.emit(name, "/>", &mut self.s); }
        pub fn empty0(&mut self, name: &str) { self.s.extend(["<", name, "/>"]); }
        pub fn val(&mut self, name: &str, v: impl ToString) { A::new().a("val", v).emit(name, "/>", &mut self.s); }
        pub fn text(&mut self, t: &str) { self.s.push_str(&escape_text(t)); }
        pub fn elt(&mut self, name: &str, t: &str) { self.open0(name); self.text(t); self.close(name); }
        pub fn raw(&mut self, xml: &str) { self.s.push_str(xml); }
        pub fn finish(self) -> Vec<u8> { self.s.into_bytes() }
    }
}
