//! Collected stories and the sorted registry view.

use std::sync::OnceLock;

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
}

/// One collected story.
#[derive(Debug)]
pub struct StoryEntry {
    /// The story's identity and documentation.
    pub meta: StoryMeta,
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

#[cfg(test)]
mod tests {
    use super::*;

    inventory::submit! {
        StoryEntry {
            meta: StoryMeta {
                group: "Zebra",
                component: "Card",
                name: "Sale",
                description: "",
                frame: Frame::Padded,
                source_file: file!(),
            }
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
            }
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
            }
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
}
