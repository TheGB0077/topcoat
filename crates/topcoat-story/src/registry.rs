//! Collected stories and the sorted registry view.

use std::sync::OnceLock;

use topcoat_core::context::Cx;
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
        assert_eq!(
            order,
            vec![
                ("Apple", "Card", "Default"),
                ("Apple", "Card", "Sold out"),
                ("Zebra", "Card", "Sale"),
            ]
        );
    }

    #[test]
    fn find_matches_a_full_story_path() {
        let entry = find("Apple", "Card", "Default").expect("the story is registered");
        assert_eq!(entry.meta.name, "Default");
        assert!(find("Apple", "Card", "Missing").is_none());
    }
}
