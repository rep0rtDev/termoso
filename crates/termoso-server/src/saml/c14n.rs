//! Canonical XML 1.0 and Exclusive XML Canonicalization 1.0 over a
//! [`roxmltree`] subtree — the two algorithms every SAML IdP uses for
//! `ds:SignedInfo` and for the enveloped-signature reference.
//!
//! Only the parts XML-DSig needs are implemented: canonicalizing an element
//! subtree (optionally without one excluded descendant, the `ds:Signature`
//! itself), with or without comments. Document subsets selected by XPath are
//! not supported; the DSig layer never produces them.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use roxmltree::{Node, NodeType};

pub const INCLUSIVE: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
pub const INCLUSIVE_WITH_COMMENTS: &str =
    "http://www.w3.org/TR/2001/REC-xml-c14n-20010315#WithComments";
pub const INCLUSIVE_1_1: &str = "http://www.w3.org/2006/12/xml-c14n11";
pub const INCLUSIVE_1_1_WITH_COMMENTS: &str = "http://www.w3.org/2006/12/xml-c14n11#WithComments";
pub const EXCLUSIVE: &str = "http://www.w3.org/2001/10/xml-exc-c14n#";
pub const EXCLUSIVE_WITH_COMMENTS: &str = "http://www.w3.org/2001/10/xml-exc-c14n#WithComments";

const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Inclusive,
    Exclusive,
}

#[derive(Debug, Clone)]
pub struct Method {
    pub mode: Mode,
    pub with_comments: bool,
    /// `InclusiveNamespaces/@PrefixList` for exclusive c14n (`#default` = the
    /// default namespace).
    pub inclusive_prefixes: Vec<String>,
}

impl Method {
    pub fn from_algorithm(uri: &str, inclusive_prefixes: Vec<String>) -> Option<Self> {
        let (mode, with_comments) = match uri {
            INCLUSIVE | INCLUSIVE_1_1 => (Mode::Inclusive, false),
            INCLUSIVE_WITH_COMMENTS | INCLUSIVE_1_1_WITH_COMMENTS => (Mode::Inclusive, true),
            EXCLUSIVE => (Mode::Exclusive, false),
            EXCLUSIVE_WITH_COMMENTS => (Mode::Exclusive, true),
            _ => return None,
        };
        Some(Self {
            mode,
            with_comments,
            inclusive_prefixes,
        })
    }
}

/// Canonicalize the subtree rooted at `root`. `exclude` (and everything below
/// it) is dropped from the output — this is the enveloped-signature transform.
pub fn canonicalize(root: Node, exclude: Option<Node>, method: &Method) -> String {
    let mut out = String::new();
    let mut w = Writer {
        method,
        exclude: exclude.map(|n| n.id()),
        out: &mut out,
    };
    // Namespaces rendered by output ancestors: prefix ("" = default) → URI.
    // Outside the subtree nothing has been rendered, i.e. only the implicit
    // empty default namespace.
    let mut rendered: BTreeMap<&str, &str> = BTreeMap::new();
    rendered.insert("", "");
    w.element(root, &rendered);
    out
}

struct Writer<'a, 'o> {
    method: &'a Method,
    exclude: Option<roxmltree::NodeId>,
    out: &'o mut String,
}

impl Writer<'_, '_> {
    fn element<'a>(&mut self, node: Node<'a, '_>, rendered: &BTreeMap<&'a str, &'a str>) {
        if Some(node.id()) == self.exclude {
            return;
        }
        let tag = node.tag_name();
        let elem_prefix = node
            .lookup_prefix(tag.namespace().unwrap_or(""))
            .unwrap_or("");
        // The prefix that appeared in the source is what c14n renders, so prefer
        // the literal one from the QName over a reverse lookup.
        let elem_prefix = qname_prefix(node).unwrap_or(elem_prefix);

        // 1. Namespace declarations for this element.
        let mut ns_out: BTreeMap<&'a str, &'a str> = BTreeMap::new();
        let mut in_scope: BTreeMap<&'a str, &'a str> = BTreeMap::new();
        for ns in node.namespaces() {
            in_scope.insert(ns.name().unwrap_or(""), ns.uri());
        }
        if !in_scope.contains_key("") {
            in_scope.insert("", "");
        }
        match self.method.mode {
            Mode::Inclusive => {
                for (prefix, uri) in &in_scope {
                    if *prefix == "xml" {
                        continue;
                    }
                    if rendered.get(prefix) != Some(uri) {
                        ns_out.insert(prefix, uri);
                    }
                }
            }
            Mode::Exclusive => {
                let mut used: Vec<&str> = vec![elem_prefix];
                for a in node.attributes() {
                    if let Some(p) = attr_prefix(node, &a)
                        && p != "xml"
                    {
                        used.push(p);
                    }
                }
                for p in &self.method.inclusive_prefixes {
                    let p = if p == "#default" { "" } else { p.as_str() };
                    if let Some((k, _)) = in_scope.get_key_value(p) {
                        used.push(k);
                    }
                }
                for prefix in used {
                    let uri = in_scope.get(prefix).copied().unwrap_or("");
                    if prefix.is_empty() && uri.is_empty() && rendered.get("") == Some(&"") {
                        continue;
                    }
                    if rendered.get(prefix) != Some(&uri) {
                        ns_out.insert(prefix, uri);
                    }
                }
            }
        }

        // 2. Attributes sorted by (namespace URI, local name); un-namespaced first.
        let mut attrs: Vec<(&str, &str, &str, &str)> = node
            .attributes()
            .map(|a| {
                (
                    a.namespace().unwrap_or(""),
                    a.name(),
                    attr_prefix(node, &a).unwrap_or(""),
                    a.value(),
                )
            })
            .collect();
        attrs.sort_by(|x, y| (x.0, x.1).cmp(&(y.0, y.1)));

        self.out.push('<');
        push_qname(self.out, elem_prefix, tag.name());
        for (prefix, uri) in &ns_out {
            if prefix.is_empty() {
                let _ = write!(self.out, " xmlns=\"{}\"", escape_attr(uri));
            } else {
                let _ = write!(self.out, " xmlns:{prefix}=\"{}\"", escape_attr(uri));
            }
        }
        for (ns, name, prefix, value) in &attrs {
            self.out.push(' ');
            let prefix = if *ns == XML_NS { "xml" } else { prefix };
            push_qname(self.out, prefix, name);
            let _ = write!(self.out, "=\"{}\"", escape_attr(value));
        }
        self.out.push('>');

        let mut child_rendered = rendered.clone();
        for (p, u) in &ns_out {
            child_rendered.insert(p, u);
        }
        for child in node.children() {
            self.node(child, &child_rendered);
        }

        self.out.push_str("</");
        push_qname(self.out, elem_prefix, tag.name());
        self.out.push('>');
    }

    fn node<'a>(&mut self, node: Node<'a, '_>, rendered: &BTreeMap<&'a str, &'a str>) {
        match node.node_type() {
            NodeType::Element => self.element(node, rendered),
            NodeType::Text => self.out.push_str(&escape_text(node.text().unwrap_or(""))),
            NodeType::Comment => {
                if self.method.with_comments {
                    let _ = write!(self.out, "<!--{}-->", node.text().unwrap_or(""));
                }
            }
            NodeType::PI => {
                if let Some(pi) = node.pi() {
                    self.out.push_str("<?");
                    self.out.push_str(pi.target);
                    if let Some(v) = pi.value {
                        self.out.push(' ');
                        self.out.push_str(v);
                    }
                    self.out.push_str("?>");
                }
            }
            NodeType::Root => {}
        }
    }
}

fn push_qname(out: &mut String, prefix: &str, local: &str) {
    if !prefix.is_empty() {
        out.push_str(prefix);
        out.push(':');
    }
    out.push_str(local);
}

/// Prefix as written in the source QName of an element (`ds:Signature` → `ds`).
fn qname_prefix<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    let text = node.document().input_text().get(node.range())?;
    let qname = text
        .strip_prefix('<')?
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()?;
    qname.split_once(':').map(|(p, _)| p)
}

fn attr_prefix<'a>(node: Node<'a, '_>, a: &roxmltree::Attribute<'a, '_>) -> Option<&'a str> {
    let text = node.document().input_text().get(a.range_qname())?;
    text.split_once(':').map(|(p, _)| p)
}

fn escape_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\r' => o.push_str("&#xD;"),
            c => o.push(c),
        }
    }
    o
}

fn escape_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '"' => o.push_str("&quot;"),
            '\t' => o.push_str("&#x9;"),
            '\n' => o.push_str("&#xA;"),
            '\r' => o.push_str("&#xD;"),
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c14n(xml: &str, method: &Method) -> String {
        let doc = roxmltree::Document::parse(xml).unwrap();
        canonicalize(doc.root_element(), None, method)
    }

    fn exc() -> Method {
        Method::from_algorithm(EXCLUSIVE, vec![]).unwrap()
    }
    fn inc() -> Method {
        Method::from_algorithm(INCLUSIVE, vec![]).unwrap()
    }

    #[test]
    fn attributes_sorted_and_escaped() {
        let out = c14n(
            r#"<a z="1" b="x &amp; &lt;y&gt; &quot;q&quot;" xmlns:n="urn:n" n:a="2"><!--c--><b/></a>"#,
            &exc(),
        );
        assert_eq!(
            out,
            r#"<a xmlns:n="urn:n" b="x &amp; &lt;y> &quot;q&quot;" z="1" n:a="2"><b></b></a>"#
        );
    }

    #[test]
    fn comments_kept_when_requested() {
        let m = Method::from_algorithm(EXCLUSIVE_WITH_COMMENTS, vec![]).unwrap();
        assert_eq!(c14n("<a><!-- x --></a>", &m), "<a><!-- x --></a>");
    }

    #[test]
    fn exclusive_drops_unused_namespaces() {
        let xml = r#"<p:root xmlns:p="urn:p" xmlns:q="urn:q" xmlns:r="urn:r"><q:child r:attr="1"/><p:only/></p:root>"#;
        assert_eq!(
            c14n(xml, &exc()),
            r#"<p:root xmlns:p="urn:p"><q:child xmlns:q="urn:q" xmlns:r="urn:r" r:attr="1"></q:child><p:only></p:only></p:root>"#
        );
    }

    #[test]
    fn exclusive_inclusive_prefix_list() {
        let m = Method::from_algorithm(EXCLUSIVE, vec!["xs".into(), "#default".into()]).unwrap();
        let xml = r#"<a xmlns="urn:d" xmlns:xs="urn:xs" xmlns:u="urn:u"><b xs:type="t"/></a>"#;
        assert_eq!(
            c14n(xml, &m),
            r#"<a xmlns="urn:d" xmlns:xs="urn:xs"><b xs:type="t"></b></a>"#
        );
    }

    #[test]
    fn inclusive_renders_all_in_scope_once() {
        let xml = r#"<a xmlns:z="urn:z" xmlns:b="urn:b"><c xmlns:b="urn:b" xmlns:y="urn:y"/></a>"#;
        assert_eq!(
            c14n(xml, &inc()),
            r#"<a xmlns:b="urn:b" xmlns:z="urn:z"><c xmlns:y="urn:y"></c></a>"#
        );
    }

    #[test]
    fn default_namespace_and_undeclaration() {
        let xml = r#"<a xmlns="urn:a"><b xmlns=""><c/></b></a>"#;
        assert_eq!(
            c14n(xml, &exc()),
            r#"<a xmlns="urn:a"><b xmlns=""><c></c></b></a>"#
        );
    }

    #[test]
    fn exclude_subtree() {
        let doc = roxmltree::Document::parse("<a><b>x</b><s><t/></s>y</a>").unwrap();
        let s = doc.descendants().find(|n| n.has_tag_name("s")).unwrap();
        assert_eq!(
            canonicalize(doc.root_element(), Some(s), &exc()),
            "<a><b>x</b>y</a>"
        );
    }

    #[test]
    fn text_escaping_and_cr() {
        assert_eq!(
            c14n("<a>1 &lt; 2 &amp; 3 &gt; &#xD;\n</a>", &exc()),
            "<a>1 &lt; 2 &amp; 3 &gt; &#xD;\n</a>"
        );
        assert_eq!(
            c14n("<a x=\"tab&#x9;nl&#xA;\"/>", &exc()),
            "<a x=\"tab&#x9;nl&#xA;\"></a>"
        );
    }

    #[test]
    fn xml_prefixed_attributes() {
        assert_eq!(
            c14n(r#"<a xml:lang="en" b="1"/>"#, &exc()),
            r#"<a b="1" xml:lang="en"></a>"#
        );
    }

    /// Expected strings produced by libxml2 (`lxml.etree.tostring(method="c14n")`).
    #[test]
    fn matches_libxml2() {
        let cases = [
            (
                r#"<a:root xmlns:a="urn:a" xmlns:b="urn:b" xmlns="urn:d"><b:x xmlns:a="urn:a" a:attr="1"><y/></b:x><a:z xmlns:c="urn:c" c:q="v" b:p="w"/></a:root>"#,
                r#"<a:root xmlns:a="urn:a"><b:x xmlns:b="urn:b" a:attr="1"><y xmlns="urn:d"></y></b:x><a:z xmlns:b="urn:b" xmlns:c="urn:c" b:p="w" c:q="v"></a:z></a:root>"#,
                r#"<a:root xmlns="urn:d" xmlns:a="urn:a" xmlns:b="urn:b"><b:x a:attr="1"><y></y></b:x><a:z xmlns:c="urn:c" b:p="w" c:q="v"></a:z></a:root>"#,
            ),
            (
                r#"<r xmlns:p="urn:p"><p:e xmlns:p="urn:p2"><p:f/></p:e><p:g/></r>"#,
                r#"<r><p:e xmlns:p="urn:p2"><p:f></p:f></p:e><p:g xmlns:p="urn:p"></p:g></r>"#,
                r#"<r xmlns:p="urn:p"><p:e xmlns:p="urn:p2"><p:f></p:f></p:e><p:g></p:g></r>"#,
            ),
            (
                r#"<r xmlns="urn:d"><e xmlns=""><f xmlns="urn:d"/></e></r>"#,
                r#"<r xmlns="urn:d"><e xmlns=""><f xmlns="urn:d"></f></e></r>"#,
                r#"<r xmlns="urn:d"><e xmlns=""><f xmlns="urn:d"></f></e></r>"#,
            ),
        ];
        for (input, exclusive, inclusive) in cases {
            assert_eq!(c14n(input, &exc()), exclusive, "exclusive: {input}");
            assert_eq!(c14n(input, &inc()), inclusive, "inclusive: {input}");
        }
    }

    #[test]
    fn processing_instruction() {
        assert_eq!(c14n("<a><?pi data?></a>", &exc()), "<a><?pi data?></a>");
    }
}
