//! The story dashboard: sidebar, canvas, controls, and source panels.
//!
//! Register the dashboard's routes on a router with
//! [`dashboard`](RouterBuilderDashboardExt::dashboard). The routes live at
//! the root of that router, so a workshop takes no route of its own from an
//! application that happens to link this crate. Chrome uses inline styles
//! rather than the application's stylesheet, so the dashboard looks right
//! in any app while stories render in the app's own styling for production
//! fidelity.

use std::{fmt::Write as _, path::Path};

use topcoat_core::{context::Cx, error::Result};
use topcoat_router::{
    Body, HeaderValue, Method, PageFn, RouteFn, RouteFuture, RouterBuilder, content::Json,
    error::RouterErrorExt, header, path_param, request::uri, response::IntoResponse,
};
use topcoat_router_macro::path_param;
use topcoat_view::{BoxView, Child, HoistView, View, internal::ThenView};
use topcoat_view_macro::{component, view};

use super::{Frame, ParamKind, StoryEntry, find, stories};

/// Percent-encodes one URL path segment, leaving unreserved characters.
fn encode_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.bytes() {
        if matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// The canonical dashboard URL for a story: the URL is the saved state.
fn story_url(group: &str, component: &str, name: &str) -> String {
    format!(
        "/{}/{}/{}",
        encode_segment(group),
        encode_segment(component),
        encode_segment(name)
    )
}

/// The source of the dev server's update script, when the dashboard runs
/// under `topcoat story run`. Renders nothing when there is no dev server.
fn dev_script() -> Option<String> {
    std::env::var("TOPCOAT_DEV_URL")
        .ok()
        .map(|base| format!("{base}/dev.js"))
}

/// One sidebar group: a name holding components.
struct SidebarGroup {
    name: &'static str,
    components: Vec<SidebarComponent>,
}

/// One sidebar component: a name holding story names.
struct SidebarComponent {
    name: &'static str,
    stories: Vec<&'static str>,
}

/// The registry grouped for the sidebar, in registry order.
fn sidebar_tree() -> Vec<SidebarGroup> {
    let mut tree: Vec<SidebarGroup> = Vec::new();
    for entry in stories() {
        let meta = &entry.meta;
        let group =
            if let Some(group_index) = tree.iter().position(|group| group.name == meta.group) {
                &mut tree[group_index]
            } else {
                tree.push(SidebarGroup {
                    name: meta.group,
                    components: Vec::new(),
                });
                tree.last_mut().expect("group was pushed")
            };
        let component = if let Some(component_index) = group
            .components
            .iter()
            .position(|component| component.name == meta.component)
        {
            &mut group.components[component_index]
        } else {
            group.components.push(SidebarComponent {
                name: meta.component,
                stories: Vec::new(),
            });
            group.components.last_mut().expect("component was pushed")
        };
        component.stories.push(meta.name);
    }
    tree
}

/// The raw query value for a key, if the request carries one.
fn query_value<'a>(query: Option<&'a str>, key: &str) -> Option<&'a str> {
    query?
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value)
}

/// Reads a registry source file for the source panel.
///
/// Registry paths are workspace-relative, while the server's working
/// directory is not one fixed place, so this searches the working
/// directory's and the running binary's ancestor directories for the
/// file. Installed binaries far from their sources find nothing, and
/// the panel says so.
fn read_source(source_file: &str) -> Option<String> {
    let direct = Path::new(source_file);
    if let Ok(source) = std::fs::read_to_string(direct) {
        return Some(source);
    }
    let roots = std::env::current_dir()
        .ok()
        .into_iter()
        .chain(std::env::current_exe().ok());
    for root in roots {
        for ancestor in root.ancestors() {
            if let Ok(source) = std::fs::read_to_string(ancestor.join(source_file)) {
                return Some(source);
            }
        }
    }
    None
}

/// The dashboard shell: sidebar registry tree plus the page content.
#[component]
async fn chrome(title: &str, #[default] child: Child<'_>) -> Result<impl View> {
    let tree = sidebar_tree();
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>
                    (title)
                    " — designbook"
                </title>
                if let Some(src) = dev_script() {
                    <script src=(src)></script>
                }
            </head>
            <body
                style="margin:0;display:flex;min-height:100vh;font-family:system-ui,sans-serif;background:#faf8f5;color:#2c221e"
            >
                <nav
                    style="width:260px;flex:none;border-right:1px solid #e5e0d8;padding:16px;overflow-y:auto"
                >
                    <p
                        style="margin:0 0 12px;font-weight:800;letter-spacing:0.08em;font-size:12px;text-transform:uppercase"
                    >
                        <a href="/" style="color:inherit;text-decoration:none">
                            "Designbook"
                        </a>
                    </p>
                    for group in &tree {
                        <p
                            style="margin:12px 0 4px;font-size:12px;font-weight:700;color:#8a7a6d"
                        >
                            (group.name)
                        </p>
                        for component in &group.components {
                            <p style="margin:8px 0 2px;font-size:13px;font-weight:600">
                                (component.name)
                            </p>
                            <ul style="margin:0;padding:0;list-style:none">
                                for name in &component.stories {
                                    <li>
                                        <a
                                            href=(story_url(group.name, component.name, name))
                                            style="display:block;padding:2px 8px;font-size:13px;color:#2c221e;text-decoration:none;border-radius:6px"
                                        >
                                            (name)
                                        </a>
                                    </li>
                                }
                            </ul>
                        }
                    }
                </nav>
                <main style="flex:1;padding:24px;max-width:1100px">(child)</main>
            </body>
        </html>
    })
}

/// Frames a story canvas per its frame choice.
#[component]
async fn framed(frame: Frame, #[default] child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        match frame {
            Frame::Padded => {
                <div
                    style="background:#fff;border:1px solid #e5e0d8;border-radius:12px;padding:24px"
                >
                    (child)
                </div>
            }
            Frame::Bare => {
                <div>(child)</div>
            }
            Frame::Dark => {
                <div style="background:#1c2b24;border-radius:12px;padding:24px">
                    (child)
                </div>
            }
            Frame::FullBleed => {
                <div style="margin:0 -24px">(child)</div>
            }
        }
    })
}

/// One control for a query key.
#[component]
async fn control(
    name: &'static str,
    kind: ParamKind,
    current: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        <div style="display:flex;align-items:center;gap:8px;margin-bottom:8px">
            <span style="width:140px;font-size:13px;font-weight:600">(name)</span>
            match kind {
                ParamKind::Select(options) => {
                    <select
                        name=(name)
                        onchange="this.form.submit()"
                        style="flex:1;padding:4px 8px;font-size:13px;border:1px solid #e5e0d8;border-radius:6px;background:#fff"
                    >
                        for option in options {
                            <option
                                value=(option)
                                selected=(current.as_deref() == Some(*option))
                            >
                                (option)
                            </option>
                        }
                    </select>
                }
                ParamKind::Toggle => {
                    <input
                        type="checkbox"
                        name=(name)
                        value="true"
                        checked=(current.as_deref() == Some("true"))
                        onchange="this.form.submit()"
                    >
                }
                ParamKind::Text => {
                    <input
                        type="text"
                        name=(name)
                        value=(current)
                        onchange="this.form.submit()"
                        style="flex:1;padding:4px 8px;font-size:13px;border:1px solid #e5e0d8;border-radius:6px"
                    >
                }
            }
        </div>
    })
}

/// The dashboard index.
fn index_view(cx: &Cx) -> impl View {
    let empty = stories().is_empty();
    view! {
        cx =>
        ((header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        chrome(
            title: "Index",
            <h1 style="margin-top:0">"Stories"</h1>
            if empty {
                <p>"No stories registered yet. Add a #[story] to a component."</p>
            } else {
                <p>"Pick a story from the sidebar."</p>
            }
        )
    }
}

path_param!(group);
path_param!(component);
path_param!(story_name);

/// One story, with its controls and source panels.
fn story_page_view(cx: &Cx) -> Result<impl View> {
    let group = path_param::<Group>(cx);
    let component = path_param::<Component>(cx);
    let name = path_param::<StoryName>(cx);
    let entry: &'static StoryEntry = find(group, component, name).ok_or_not_found()?;
    let query = uri(cx).query().map(str::to_owned);

    Ok(view! {
        cx =>
        ((header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        chrome(
            title: entry.meta.name,
            <h1 style="margin-top:0">(entry.meta.name)</h1>
            <p style="color:#8a7a6d">
                (entry.meta.group)
                " / "
                (entry.meta.component)
            </p>
            if !entry.meta.description.is_empty() {
                <p>(entry.meta.description)</p>
            }

            framed(frame: entry.meta.frame, ((entry.render)(cx)))

            if !entry.meta.params.is_empty() {
                <section style="margin-top:24px">
                    <h2>"Controls"</h2>
                    <form method="get" action="">
                        for param in entry.meta.params {
                            control(
                                name: param.name,
                                kind: param.kind,
                                current: query_value(query.as_deref(), param.name).map(
                                    str::to_owned,
                                )
                            )
                        }
                        <button
                            type="submit"
                            style="margin-top:8px;padding:6px 16px;font-size:13px;font-weight:700;border:1px solid #e5e0d8;border-radius:6px;background:#fff;cursor:pointer"
                        >
                            "Apply"
                        </button>
                    </form>
                </section>
            }

            <section style="margin-top:24px">
                <h2>"Source"</h2>
                <p style="color:#8a7a6d;font-size:13px">(entry.meta.source_file)</p>
                match read_source(entry.meta.source_file) {
                    Some(source) => {
                        <pre
                            style="background:#2c221e;color:#faf8f5;padding:16px;border-radius:12px;overflow-x:auto;font-size:12px"
                        >
                            (source)
                        </pre>
                    }
                    None => {
                        <p>"Source is unavailable away from the workspace."</p>
                    }
                }
            </section>
        )
    })
}

/// One manifest row: a story's identity and canonical URL.
#[cfg_attr(docsrs, doc(cfg(feature = "router")))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ManifestEntry {
    /// Sidebar group.
    pub group: &'static str,
    /// Component name inside the group.
    pub component: &'static str,
    /// Story name inside the component.
    pub name: &'static str,
    /// Canonical dashboard URL: the URL is the saved state.
    pub url: String,
}

/// `GET /manifest.json` — every story URL, for snapshot and pixel test
/// runners to walk without hand-maintained lists.
fn manifest(cx: &Cx, _body: Body) -> RouteFuture<'_> {
    Box::pin(async move {
        let entries: Vec<ManifestEntry> = stories()
            .iter()
            .map(|entry| ManifestEntry {
                group: entry.meta.group,
                component: entry.meta.component,
                name: entry.meta.name,
                url: story_url(entry.meta.group, entry.meta.component, entry.meta.name),
            })
            .collect();
        Json(entries).into_response(cx)
    })
}

/// Renders one dashboard page from the view `render` resolves to, with the
/// parts it hoists collected ahead of its content.
fn page<'cx, V: View + 'cx>(render: impl Future<Output = Result<V>> + Send + 'cx) -> BoxView<'cx> {
    Box::pin(HoistView::new(ThenView::new(render)))
}

/// `GET /` — the dashboard index.
fn index_page(cx: &Cx, _body: Body) -> BoxView<'_> {
    page(async move { Ok(index_view(cx)) })
}

/// `GET /{group}/{component}/{story}` — one story.
fn story_page_route(cx: &Cx, _body: Body) -> BoxView<'_> {
    page(async move { story_page_view(cx) })
}

/// Registers the story dashboard's routes on a router builder.
///
/// The routes sit at the root of the router they are registered on: the
/// index at `/`, one page per story at `/{group}/{component}/{story}`, and
/// the story manifest at `/manifest.json`.
pub trait RouterBuilderDashboardExt {
    /// Registers the dashboard's routes.
    #[must_use]
    fn dashboard(self) -> Self;
}

impl RouterBuilderDashboardExt for RouterBuilder {
    fn dashboard(self) -> Self {
        self.page(PageFn::new(Method::GET, "/", index_page))
            .page(PageFn::new(
                Method::GET,
                "/{group}/{component}/{story_name}",
                story_page_route,
            ))
            .route(RouteFn::new(Method::GET, "/manifest.json", manifest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_reserved_characters_in_story_urls() {
        assert_eq!(encode_segment("Sold out"), "Sold%20out");
        assert_eq!(encode_segment("plain-1.0~x_y"), "plain-1.0~x_y");
        assert_eq!(
            story_url("Shop", "Badge", "Sold out"),
            "/Shop/Badge/Sold%20out"
        );
    }

    #[test]
    fn reads_registry_sources_from_ancestor_directories() {
        let source = read_source("crates/topcoat-story/src/dashboard.rs").expect("source resolves");
        assert!(source.contains("source panel"));
    }
}
