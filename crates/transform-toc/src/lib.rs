//! First-party table-of-contents transformer plugin.
//!
//! Collects each document's section tree, then inserts a nested list of
//! links to the sections into the document's first section, right after its
//! title. The first section is taken as the page title, so the list holds
//! its subsections followed by any later top-level sections. Documents with
//! nothing to list are left alone.
//!
//! Settings (`.config/lsdocs/<name>/settings.toml`): `depth` is the
//! deepest heading level listed (default 3).

use lsdocs_plugin_sdk::doctree::{
    Custom, Link, List, ListItem, Node, Paragraph, Section, plain_text, text,
};
use lsdocs_plugin_sdk::protocol::{
    ParseTransformEachParams, ParseTransformEachResult, RenderTransformEachParams,
    RenderTransformEachResult,
};
use lsdocs_plugin_sdk::{Context, Plugin, PluginResult, Transformer};
use serde_json::{Value, json};

/// The plugin, as registered with the host.
pub fn plugin() -> Plugin {
    Plugin::new("transform-toc", env!("CARGO_PKG_VERSION"))
        .settings(|c| lsdocs_plugin_sdk::config::settings_toml_keys(c, &["depth"]))
        .transformer(Toc)
        .cacheable()
}

/// The table-of-contents transformer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Toc;

impl Transformer for Toc {
    const READS: bool = true;

    fn nodes(&self) -> Vec<String> {
        vec!["section".into()]
    }

    async fn read(
        &self,
        params: ParseTransformEachParams,
        ctx: &Context,
    ) -> PluginResult<ParseTransformEachResult> {
        let depth = ctx
            .settings()
            .get("depth")
            .and_then(Value::as_u64)
            .unwrap_or(3);
        Ok(ParseTransformEachResult {
            data: json!({ "sections": entries(&params.document.children, depth) }),
            diagnostics: vec![],
            dependencies: vec![],
        })
    }

    async fn apply(
        &self,
        params: RenderTransformEachParams,
        _ctx: &Context,
    ) -> PluginResult<RenderTransformEachResult> {
        let mut document = params.document;
        // Use what the collect pass gathered for this document.
        let entries = params
            .collected
            .get(&document.source)
            .and_then(|d| d.get("sections"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some((first, rest)) = entries.split_first() {
            let mut items = first["children"].as_array().cloned().unwrap_or_default();
            items.extend(rest.iter().cloned());
            let title_section = document.children.iter_mut().find_map(|n| match n {
                Node::Section(s) => Some(s),
                _ => None,
            });
            if !items.is_empty()
                && let Some(section) = title_section
            {
                let at = usize::from(matches!(section.children.first(), Some(Node::Heading(_))));
                section.children.insert(at, toc_node(&items));
            }
        }
        Ok(RenderTransformEachResult {
            document,
            diagnostics: vec![],
            dependencies: vec![],
        })
    }
}

/// The sections among `nodes`, as `{id, title, children}` entries, leaving
/// out sections whose title is deeper than `depth`.
fn entries(nodes: &[Node], depth: u64) -> Vec<Value> {
    nodes
        .iter()
        .filter_map(|node| {
            let Node::Section(Section {
                id: Some(id),
                children,
                ..
            }) = node
            else {
                return None;
            };
            let Some(Node::Heading(heading)) = children.first() else {
                return None;
            };
            if u64::from(heading.level) > depth {
                return None;
            }
            Some(json!({
                "id": id,
                "title": plain_text(&heading.children),
                "children": entries(&children[1..], depth),
            }))
        })
        .collect()
}

/// A `custom:toc` node wrapping a nested list of links.
fn toc_node(entries: &[Value]) -> Node {
    Node::Custom(Custom {
        kind: "toc".into(),
        children: vec![list(entries)],
        ..Custom::default()
    })
}

/// A list of links to `entries`, with each entry's children nested inside
/// its item.
fn list(entries: &[Value]) -> Node {
    let items = entries
        .iter()
        .map(|entry| {
            let id = entry["id"].as_str().unwrap_or_default();
            let title = entry["title"].as_str().unwrap_or_default();
            let mut children = vec![Node::Paragraph(Paragraph {
                children: vec![Node::Link(Link {
                    url: format!("#{id}"),
                    children: vec![text(title)],
                    ..Link::default()
                })],
                ..Paragraph::default()
            })];
            let nested = entry["children"].as_array().cloned().unwrap_or_default();
            if !nested.is_empty() {
                children.push(list(&nested));
            }
            ListItem {
                children,
                ..ListItem::default()
            }
            .into()
        })
        .collect();
    Node::List(List {
        children: items,
        ..List::default()
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;
    use lsdocs_plugin_sdk::doctree::{Document, Heading};
    use lsdocs_plugin_sdk::protocol::InitializeParams;
    use lsdocs_plugin_sdk::testing;

    fn section(level: u8, id: &str, children: Vec<Node>) -> Node {
        let mut all = vec![Node::Heading(Heading {
            level,
            children: vec![text(id.to_uppercase())],
            ..Heading::default()
        })];
        all.extend(children);
        Node::Section(Section {
            id: Some(id.into()),
            children: all,
            ..Section::default()
        })
    }

    /// Item titles of a list, with nested lists in brackets.
    fn outline(list: &Node) -> String {
        let Node::List(list) = list else { panic!() };
        let items: Vec<String> = list
            .children
            .iter()
            .map(|item| {
                let title = plain_text(item.children()[0].children());
                match item.children().get(1) {
                    Some(nested) => format!("{title}[{}]", outline(nested)),
                    None => title,
                }
            })
            .collect();
        items.join(" ")
    }

    async fn run(depth: u64, children: Vec<Node>) -> Document {
        let mut doc = Document::new("a.md");
        doc.children = children;
        let init = lsdocs_plugin_sdk::Setup::with_settings(
            InitializeParams::default(),
            json!({ "depth": depth }),
        );
        let mut out = testing::run_transform(&plugin(), &init, vec![doc])
            .await
            .unwrap();
        out.remove(0).0
    }

    #[tokio::test]
    async fn lists_the_section_tree_after_the_title() {
        let doc = run(
            3,
            vec![
                section(
                    1,
                    "top",
                    vec![
                        section(2, "a", vec![section(3, "b", vec![section(4, "c", vec![])])]),
                        section(2, "d", vec![]),
                    ],
                ),
                section(1, "e", vec![]),
            ],
        )
        .await;
        let Node::Section(top) = &doc.children[0] else {
            panic!()
        };
        let Node::Custom(toc) = &top.children[1] else {
            panic!("{top:?}")
        };
        assert_eq!(toc.kind, "toc");
        assert_eq!(outline(&toc.children[0]), "A[B] D E");
    }

    #[tokio::test]
    async fn a_lone_title_gets_no_toc() {
        let doc = run(3, vec![section(1, "top", vec![section(4, "deep", vec![])])]).await;
        assert!(!doc.descendants().any(|n| n.kind() == "custom:toc"));
    }
}
