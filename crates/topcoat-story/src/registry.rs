//! Collected stories and the sorted registry view.

use std::sync::OnceLock;

use topcoat_core::context::{Cx, try_request_context};
use topcoat_view::BoxView;

/// One control in a story's controls panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamDef {
    /// Query key the control reads and writes.
    pub name: &'static str,
    /// The control for the key.
    pub kind: ParamKind,
}

/// The control for a [`ParamDef`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// Drop-down over the listed values.
    Select(&'static [&'static str]),
    /// On/off checkbox, submitted as `true` when checked.
    Toggle,
    /// Free text input.
    Text,
}

/// How the gallery frames a story on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// Padded card on the gallery background.
    Padded,
    /// No wrapper; the story owns its spacing.
    Bare,
    /// Dark backdrop for light-on-dark components.
    Dark,
    /// Edge to edge, with no card and no padding.
    FullBleed,
}

/// Identity and documentation for one story.
///
/// The `#[story]` macro builds this from its arguments, the story
/// function's doc comment, and the file that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryMeta {
    /// Sidebar group, such as `"Catalog"`.
    pub group: &'static str,
    /// Component name inside the group, such as `"DealCard"`.
    pub component: &'static str,
    /// Story name inside the component, such as `"Default"`.
    pub name: &'static str,
    /// The story function's doc comment, shown in the sidebar.
    pub description: &'static str,
    /// Canvas framing for the story.
    pub frame: Frame,
    /// Path of the file holding the story, for the source panel.
    pub source_file: &'static str,
    /// Controls panel schema; empty when the story takes no controls.
    pub params: &'static [ParamDef],
}

/// The story's control values, set by whoever renders the story.
///
/// The dashboard sets this from the request query on full page loads
/// and from the live control signals on shard re-renders, so stories
/// read one source either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryQuery(pub String);

/// Reads a story's controls: the dashboard-provided query if the story
/// renders there, defaults otherwise.
///
/// Stories declare their controls with an ordinary deserializable struct
/// of optional fields and fall back per struct, not per field: a missing
/// or malformed query parses to `Q::default()`. Outside the dashboard,
/// where no query is provided, that is also what you get.
#[must_use]
pub fn controls<Q>(cx: &Cx) -> Q
where
    Q: serde::de::DeserializeOwned + Default,
{
    try_request_context::<StoryQuery>(cx)
        .map(|query| query.0.clone())
        .and_then(|query| serde_urlencoded::from_str(&query).ok())
        .unwrap_or_default()
}

/// Renders a story's canvas for the gallery.
pub type StoryRenderFn = for<'a> fn(&'a Cx) -> BoxView<'a>;

/// One collected story.
pub struct StoryEntry {
    /// The story's identity and documentation.
    pub meta: StoryMeta,
    /// Renders the story.
    pub render: StoryRenderFn,
}

inventory::collect!(StoryEntry);

static SORTED: OnceLock<Vec<&'static StoryEntry>> = OnceLock::new();

/// Every collected story, sorted by group, component, then story name.
///
/// Sorting keeps sidebar order and story URLs stable regardless of link
/// order.
pub fn stories() -> &'static [&'static StoryEntry] {
    SORTED.get_or_init(|| {
        let mut entries: Vec<&'static StoryEntry> =
            inventory::iter::<StoryEntry>.into_iter().collect();
        entries.sort_by(|a, b| {
            (a.meta.group, a.meta.component, a.meta.name).cmp(&(
                b.meta.group,
                b.meta.component,
                b.meta.name,
            ))
        });
        entries
    })
}

/// The story for a gallery URL: the entry whose group, component, and
/// story name all match.
#[must_use]
pub fn find(group: &str, component: &str, name: &str) -> Option<&'static StoryEntry> {
    stories()
        .iter()
        .find(|entry| {
            entry.meta.group == group
                && entry.meta.component == component
                && entry.meta.name == name
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use topcoat_view::ViewExt;

    use super::*;

    #[derive(Debug, Default, PartialEq, serde::Deserialize)]
    struct CardQuery {
        tone: Option<String>,
        glow: Option<bool>,
    }

    #[test]
    fn controls_read_the_provided_query() {
        let cx = Cx::default().with(StoryQuery("tone=loud&glow=true".to_owned()));
        assert_eq!(
            controls::<CardQuery>(&cx),
            CardQuery {
                tone: Some("loud".to_owned()),
                glow: Some(true),
            }
        );
    }

    #[test]
    fn controls_fall_back_to_defaults() {
        let cx = Cx::default();
        assert_eq!(controls::<CardQuery>(&cx), CardQuery::default());
        let cx = Cx::default().with(StoryQuery("glow=notabool".to_owned()));
        assert_eq!(controls::<CardQuery>(&cx), CardQuery::default());
    }

    fn blank(_cx: &Cx) -> BoxView<'_> {
        ().boxed()
    }

    inventory::submit! {
        StoryEntry {
            meta: StoryMeta {
                group: "Zebra",
                component: "Card",
                name: "Sale",
                description: "",
                frame: Frame::Padded,
                source_file: file!(),
                params: &[],
            },
            render: blank,
        }
    }

    inventory::submit! {
        StoryEntry {
            meta: StoryMeta {
                group: "Apple",
                component: "Card",
                name: "Sold out",
                description: "",
                frame: Frame::Bare,
                source_file: file!(),
                params: &[],
            },
            render: blank,
        }
    }

    inventory::submit! {
        StoryEntry {
            meta: StoryMeta {
                group: "Apple",
                component: "Card",
                name: "Default",
                description: "",
                frame: Frame::Padded,
                source_file: file!(),
                params: &[],
            },
            render: blank,
        }
    }

    #[test]
    fn stories_come_back_in_group_component_name_order() {
        let order: Vec<(&str, &str, &str)> = stories()
            .iter()
            .map(|entry| (entry.meta.group, entry.meta.component, entry.meta.name))
            .collect();
        assert!(order.windows(2).all(|pair| pair[0] <= pair[1]), "{order:?}");
        assert!(order.contains(&("Apple", "Card", "Default")));
    }

    #[test]
    fn find_matches_a_full_story_path() {
        let entry = find("Apple", "Card", "Default").expect("the story is registered");
        assert_eq!(entry.meta.name, "Default");
        assert!(find("Apple", "Card", "Missing").is_none());
    }
}
