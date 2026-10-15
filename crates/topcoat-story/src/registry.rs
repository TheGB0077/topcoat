//! Collected stories and the sorted registry view.

use std::sync::OnceLock;

use topcoat_core::{context::Cx, error::Error};
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
    /// Drop-down like [`ParamKind::Select`], but shared: its value rides
    /// along every dashboard navigation and the toolbar offers one
    /// switcher per key. Declare the same key with the same options in
    /// each participating story.
    Global(&'static [&'static str]),
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

/// The decoded control pairs for a query string, cached per request.
///
/// Borrowed for the lifetime of `cx`, so generated story code can merge
/// control values into borrowed props without anything escaping.
#[topcoat_core_macro::memoize]
pub fn scoped_pairs(cx: &Cx, query: &str) -> Vec<(String, String)> {
    let _ = cx;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| {
            (
                key.to_owned(),
                percent_encoding::percent_decode_str(&value.replace('+', " "))
                    .decode_utf8_lossy()
                    .into_owned(),
            )
        })
        .collect()
}

/// The decoded value for a key in already-parsed pairs.
#[must_use]
pub fn pair<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

/// Renders a story to its canvas for the given control query.
///
/// The future resolves to a view borrowing only request-lived state:
/// control values come from [`scoped_pairs`], fixtures move or borrow
/// statics, so nothing function-local escapes.
pub type StoryFuture<'a> = futures_core::future::BoxFuture<'a, Result<BoxView<'a>, Error>>;

/// Renders a story to its canvas for the given control query.
pub type StoryRenderFn = for<'a> fn(&'a Cx, String) -> StoryFuture<'a>;

/// The decoded value for a query key, with `+` as space.
#[must_use]
pub fn query_value(query: Option<&str>, key: &str) -> Option<String> {
    query?
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == key)
        .map(|(_, value)| {
            percent_encoding::percent_decode_str(&value.replace('+', " "))
                .decode_utf8_lossy()
                .into_owned()
        })
}

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

    #[test]
    fn scoped_pairs_decode_once_per_request() {
        let cx = Cx::default();
        let first = scoped_pairs(&cx, "tone=loud&nick=Gabriel+Lima");
        let second = scoped_pairs(&cx, "tone=loud&nick=Gabriel+Lima");
        assert!(std::ptr::eq(first, second));
        assert_eq!(pair(first, "tone"), Some("loud"));
        assert_eq!(pair(first, "nick"), Some("Gabriel Lima"));
        assert_eq!(pair(first, "missing"), None);
    }

    #[test]
    fn scoped_pairs_key_on_scope_and_query() {
        let cx = Cx::default();
        let scoped = cx.with("marker");
        assert_eq!(
            pair(scoped_pairs(&scoped, "tone=loud"), "tone"),
            Some("loud")
        );
        assert_eq!(pair(scoped_pairs(&cx, "tone=soft"), "tone"), Some("soft"));
    }

    fn blank(_cx: &Cx, _query: String) -> StoryFuture<'_> {
        Box::pin(async move { Ok(().boxed()) })
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
    fn query_value_decodes_keys() {
        assert_eq!(
            query_value(Some("tone=loud"), "tone"),
            Some("loud".to_owned())
        );
        assert_eq!(query_value(Some("a=1&b=2"), "b"), Some("2".to_owned()));
        assert_eq!(query_value(Some("tone=loud"), "missing"), None);
        assert_eq!(query_value(None, "tone"), None);
        assert_eq!(
            query_value(Some("name=Gabriel+Lima"), "name"),
            Some("Gabriel Lima".to_owned())
        );
        assert_eq!(
            query_value(Some("name=Gabriel%20Lima"), "name"),
            Some("Gabriel Lima".to_owned())
        );
    }

    #[test]
    fn find_matches_a_full_story_path() {
        let entry = find("Apple", "Card", "Default").expect("the story is registered");
        assert_eq!(entry.meta.name, "Default");
        assert!(find("Apple", "Card", "Missing").is_none());
    }
}
