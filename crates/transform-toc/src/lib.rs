//! First-party table-of-contents transformer plugin.
//!
//! Collects each document's section tree and writes it to the document's
//! `toc` metadata: `[{id, title, level, children}]`, for themes to show. A
//! first section with a level 1 heading is the page title and is left out
//! (its subsections are listed in its place, followed by any later
//! top-level sections).
//!
//! With `inline = true` it also inserts a nested list of links to the same
//! entries into the document's first section, right after its title.
//! Documents with nothing to list are left alone.
//!
//! Settings (`.config/lsdocs/<name>/settings.toml`): `depth` is the
//! deepest heading level listed (default 3); `inline` also inserts the
//! list into the page (default `false`).

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
        .settings(|c| {
            lsdocs_plugin_sdk::config::settings_toml_keys(
                c,
                &[
                    ("depth", lsdocs_plugin_sdk::config::Kind::Integer),
                    ("inline", lsdocs_plugin_sdk::config::Kind::Bool),
                ],
            )
        })
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
        ctx: &Context,
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
            // The page title (a level 1 first section) is not an entry.
            let mut items = if first["level"] == 1 {
                first["children"].as_array().cloned().unwrap_or_default()
            } else {
                vec![first.clone()]
            };
            items.extend(rest.iter().cloned());
            if !items.is_empty() {
                document.metadata.insert("toc".into(), Value::Array(items));
            }
            // The inline list holds the same, whatever the first section is.
            let mut items = first["children"].as_array().cloned().unwrap_or_default();
            items.extend(rest.iter().cloned());
            let inline = ctx
                .settings()
                .get("inline")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let title_section = document.children.iter_mut().find_map(|n| match n {
                Node::Section(s) => Some(s),
                _ => None,
            });
            if inline
                && !items.is_empty()
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

/// The sections among `nodes`, as `{id, title, level, children}` entries, leaving
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
                "level": heading.level,
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
        run_with(json!({ "depth": depth, "inline": true }), children).await
    }

    async fn run_with(settings: Value, children: Vec<Node>) -> Document {
        let mut doc = Document::new("a.md");
        doc.children = children;
        let init = lsdocs_plugin_sdk::Setup::with_settings(InitializeParams::default(), settings);
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
    async fn writes_toc_metadata() {
        let doc = run_with(
            json!({}),
            vec![section(2, "a", vec![section(3, "b", vec![])])],
        )
        .await;
        let toc = &doc.metadata["toc"];
        assert_eq!(toc[0]["title"], "A");
        assert_eq!(toc[0]["id"], "a");
        assert_eq!(toc[0]["level"], 2);
        assert_eq!(toc[0]["children"][0]["id"], "b");
        assert_eq!(toc[0]["children"][0]["level"], 3);
    }

    #[tokio::test]
    async fn toc_metadata_leaves_out_the_page_title() {
        let doc = run_with(
            json!({}),
            vec![section(1, "top", vec![section(2, "a", vec![])])],
        )
        .await;
        assert_eq!(doc.metadata["toc"][0]["id"], "a");
        assert_eq!(doc.metadata["toc"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn inline_list_only_when_asked() {
        let children = || vec![section(1, "top", vec![section(2, "a", vec![])])];
        let default = run_with(json!({}), children()).await;
        assert!(!default.descendants().any(|n| n.kind() == "custom:toc"));
        let asked = run_with(json!({ "inline": true }), children()).await;
        assert!(asked.descendants().any(|n| n.kind() == "custom:toc"));
    }

    #[tokio::test]
    async fn a_lone_title_gets_no_toc() {
        let doc = run(3, vec![section(1, "top", vec![section(4, "deep", vec![])])]).await;
        assert!(!doc.descendants().any(|n| n.kind() == "custom:toc"));
    }
}
