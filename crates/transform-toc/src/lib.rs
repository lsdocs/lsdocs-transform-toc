//! First-party table-of-contents transformer plugin.
//!
//! Collects each document's headings, then inserts a list of links to them
//! after the document's first heading. Documents with fewer than two
//! headings are left alone.
//!
//! Options (`[[plugins]] options = { depth = 3 }`): `depth` is the deepest
//! heading level listed (default 3).

use lsdocs_plugin_sdk::doctree::{
    Custom, Document, Heading, Link, List, ListItem, Node, Paragraph, text,
};
use lsdocs_plugin_sdk::protocol::{
    TransformApplyParams, TransformApplyResult, TransformCollectParams, TransformCollectResult,
};
use lsdocs_plugin_sdk::{Context, Plugin, PluginResult, Transformer};
use serde_json::{Value, json};

/// The plugin, as registered with the host.
pub fn plugin() -> Plugin {
    Plugin::new("transform-toc", env!("CARGO_PKG_VERSION")).transformer(Toc)
}

/// The table-of-contents transformer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Toc;

impl Transformer for Toc {
    const COLLECTS: bool = true;

    fn nodes(&self) -> Vec<String> {
        vec!["heading".into()]
    }

    async fn collect(
        &self,
        params: TransformCollectParams,
        ctx: &Context,
    ) -> PluginResult<TransformCollectResult> {
        let depth = ctx
            .options()
            .get("depth")
            .and_then(Value::as_u64)
            .unwrap_or(3);
        let headings: Vec<Value> = headings(&params.document)
            .filter(|h| u64::from(h.level) <= depth)
            .filter_map(|h| {
                Some(json!({"id": h.id.clone()?, "level": h.level, "title": plain_text(&h.children)}))
            })
            .collect();
        Ok(TransformCollectResult {
            data: json!({ "headings": headings }),
            diagnostics: vec![],
        })
    }

    async fn apply(
        &self,
        params: TransformApplyParams,
        _ctx: &Context,
    ) -> PluginResult<TransformApplyResult> {
        let mut document = params.document;
        // Use what the collect pass gathered for this document.
        let entries = params
            .collected
            .get(&document.source)
            .and_then(|d| d.get("headings"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if entries.len() >= 2 {
            let toc = toc_node(&entries[1..]);
            let first_heading = document
                .children
                .iter()
                .position(|n| matches!(n, Node::Heading(_)))
                .map_or(0, |i| i + 1);
            document.children.insert(first_heading, toc);
        }
        Ok(TransformApplyResult {
            document,
            diagnostics: vec![],
        })
    }
}

fn headings(doc: &Document) -> impl Iterator<Item = &Heading> {
    doc.descendants().filter_map(|n| match n {
        Node::Heading(h) => Some(h),
        _ => None,
    })
}

fn plain_text(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            Node::Text(t) => out.push_str(&t.value),
            Node::InlineCode(c) => out.push_str(&c.value),
            other => out.push_str(&plain_text(other.children())),
        }
    }
    out
}

/// A `custom:toc` node wrapping a list of links.
fn toc_node(entries: &[Value]) -> Node {
    let items = entries
        .iter()
        .map(|e| {
            let id = e["id"].as_str().unwrap_or_default();
            let title = e["title"].as_str().unwrap_or_default();
            ListItem {
                children: vec![Node::Paragraph(Paragraph {
                    children: vec![Node::Link(Link {
                        url: format!("#{id}"),
                        children: vec![text(title)],
                        ..Link::default()
                    })],
                    ..Paragraph::default()
                })],
                ..ListItem::default()
            }
            .into()
        })
        .collect();
    Node::Custom(Custom {
        kind: "toc".into(),
        children: vec![Node::List(List {
            children: items,
            ..List::default()
        })],
        ..Custom::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heading(level: u8, id: &str) -> Node {
        Node::Heading(Heading {
            level,
            id: Some(id.into()),
            children: vec![text(id.to_uppercase())],
            ..Heading::default()
        })
    }

    #[test]
    fn toc_lists_later_headings_after_the_first() {
        let entries = [
            json!({"id": "top", "level": 1, "title": "TOP"}),
            json!({"id": "a", "level": 2, "title": "A"}),
        ];
        let Node::Custom(toc) = toc_node(&entries[1..]) else {
            panic!()
        };
        assert_eq!(toc.kind, "toc");
        let Node::List(list) = &toc.children[0] else {
            panic!()
        };
        assert_eq!(list.children.len(), 1);
        let mut doc = Document::new("x.md");
        doc.children = vec![heading(1, "top"), heading(2, "a")];
        assert_eq!(headings(&doc).count(), 2);
    }
}
