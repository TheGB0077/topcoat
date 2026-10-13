//! The story dashboard: sidebar, canvas, controls, and source panels.
//!
//! Register the dashboard's routes on a router with
//! [`dashboard`](RouterBuilderDashboardExt::dashboard). The routes live at
//! the root of that router, so a workshop takes no route of its own from an
//! application that happens to link this crate. The shell uses inline styles
//! rather than the application's stylesheet, so the dashboard looks right
//! in any app while stories render in the app's own styling for production
//! fidelity.

use std::{fmt::Write as _, path::Path};

use topcoat_asset::AssetConfig;
use topcoat_core::{
    context::{Cx, try_app_context},
    error::Result,
};
use topcoat_router::{
    Body, HeaderValue, Method, PageFn, RouteFn, RouteFuture, RouterBuilder, content::Json,
    error::RouterErrorExt, header, path_param, request::uri, response::IntoResponse,
};
use topcoat_router_macro::path_param;
use topcoat_runtime::{Event, Signal, signal};
use topcoat_runtime_macro::shard;
use topcoat_view::{BoxView, Child, HoistView, Unescaped, View, ViewExt, internal::ThenView};
use topcoat_view_macro::{component, view};

use super::{Frame, ParamKind, StoryEntry, StoryQuery, find, stories};

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

/// Canvas width presets for the toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Viewport {
    /// The canvas takes its natural width.
    #[default]
    Full,
    /// 1024px desktop width.
    Desktop,
    /// 768px tablet width.
    Tablet,
    /// 375px phone width.
    Mobile,
}

impl Viewport {
    /// Parses the `viewport` query key, falling back to full width.
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("desktop") => Self::Desktop,
            Some("tablet") => Self::Tablet,
            Some("mobile") => Self::Mobile,
            _ => Self::Full,
        }
    }

    /// The canvas wrapper style, if the preset constrains the width.
    fn wrap(self) -> Option<&'static str> {
        match self {
            Self::Full => None,
            Self::Desktop => Some("max-width:1024px;margin:0 auto"),
            Self::Tablet => Some("max-width:768px;margin:0 auto"),
            Self::Mobile => Some("max-width:375px;margin:0 auto"),
        }
    }

    /// The query pair preserving a non-default preset.
    fn query(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Full => None,
            Self::Desktop => Some(("viewport", "desktop")),
            Self::Tablet => Some(("viewport", "tablet")),
            Self::Mobile => Some(("viewport", "mobile")),
        }
    }
}

/// Canvas backdrop presets for the toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Background {
    /// The story frame's own backdrop.
    #[default]
    Default,
    /// White paper.
    Paper,
    /// Deep pine green.
    Pine,
    /// Near-black.
    Dark,
}

impl Background {
    /// Parses the `bg` query key, falling back to the frame default.
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("paper") => Self::Paper,
            Some("pine") => Self::Pine,
            Some("dark") => Self::Dark,
            _ => Self::Default,
        }
    }

    /// The backdrop wrapper style, if the preset overrides the frame.
    fn backdrop(self) -> Option<&'static str> {
        match self {
            Self::Default => None,
            Self::Paper => Some("background:#ffffff;border-radius:12px;padding:24px"),
            Self::Pine => Some("background:#1c2b24;border-radius:12px;padding:24px"),
            Self::Dark => Some("background:#1a1a1a;border-radius:12px;padding:24px"),
        }
    }

    /// The query pair preserving a non-default preset.
    fn query(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Default => None,
            Self::Paper => Some(("bg", "paper")),
            Self::Pine => Some(("bg", "pine")),
            Self::Dark => Some(("bg", "dark")),
        }
    }
}

/// Toolbar state: the two canvas presets. Everything else in the query
/// string belongs to the story's controls or the sidebar search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Toolbar {
    viewport: Viewport,
    bg: Background,
}

impl Toolbar {
    /// Reads the toolbar keys out of a raw query string.
    fn parse(query: Option<&str>) -> Self {
        Self {
            viewport: Viewport::parse(query_value(query, "viewport")),
            bg: Background::parse(query_value(query, "bg")),
        }
    }

    /// The query suffix preserving non-default toolbar state, for links
    /// that navigate without submitting a form.
    fn suffix(self) -> String {
        let mut suffix = String::new();
        for (key, value) in self.viewport.query().into_iter().chain(self.bg.query()) {
            suffix.push(if suffix.is_empty() { '?' } else { '&' });
            suffix.push_str(key);
            suffix.push('=');
            suffix.push_str(value);
        }
        suffix
    }

    /// The toolbar keys as hidden form pairs, for the search form that
    /// submits without the toolbar selects.
    fn hidden(self) -> Vec<(&'static str, &'static str)> {
        self.viewport
            .query()
            .into_iter()
            .chain(self.bg.query())
            .collect()
    }
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

/// The registry grouped for the sidebar, in registry order. A filter
/// keeps the groups, components, and stories whose names contain it,
/// case-insensitively; an empty filter keeps everything.
fn sidebar_tree(filter: Option<&str>) -> Vec<SidebarGroup> {
    fn matches(text: &str, filter: &str) -> bool {
        text.to_lowercase().contains(&filter.to_lowercase())
    }

    let mut tree: Vec<SidebarGroup> = Vec::new();
    for entry in stories() {
        let meta = &entry.meta;
        if let Some(filter) = filter.filter(|filter| !filter.is_empty()) {
            let group_hit = matches(meta.group, filter);
            let component_hit = matches(meta.component, filter);
            let story_hit = matches(meta.name, filter);
            if !(group_hit || component_hit || story_hit) {
                continue;
            }
        }
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

/// Keeps the address bar on the live form's values without navigating.
/// Control inputs call it on change; the shard re-render answers the
/// state change, so this only records it for sharing.
const SYNC_SCRIPT: &str = "function syncStoryUrl(form){var params=new URLSearchParams(new FormData(form));var query=params.toString();history.replaceState(null,'',location.pathname+(query?'?'+query:''));}";

/// The dashboard shell: sidebar registry tree plus the page content.
/// `current` is the request path, highlighting the open story. `toolbar`
/// state rides along in sidebar links so canvas presets survive
/// navigation, and `filter` narrows the tree to matching names.
#[component]
async fn shell(
    cx: &Cx,
    title: &str,
    current: &str,
    toolbar: Toolbar,
    filter: Option<String>,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let tree = sidebar_tree(filter.as_deref());
    let suffix = toolbar.suffix();
    let filtering = filter.is_some();
    let live = try_app_context::<AssetConfig>(cx).is_some();
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
                if live {
                    <script
                        type="module"
                        src=(topcoat_runtime::SCRIPT)
                        data-topcoat-usize-bits=(usize::BITS)
                    ></script>
                }
                <script>(Unescaped::new_unchecked(SYNC_SCRIPT))</script>
            </head>
            <body
                style="margin:0;display:flex;min-height:100vh;font-family:system-ui,sans-serif;background:#faf8f5;color:#2c221e"
            >
                <nav
                    style="width:260px;flex:none;border-right:1px solid #e5e0d8;padding:16px;overflow-y:auto;position:sticky;top:0;height:100vh;box-sizing:border-box"
                >
                    <p
                        style="margin:0 0 12px;font-weight:800;letter-spacing:0.08em;font-size:12px;text-transform:uppercase"
                    >
                        <a href="/" style="color:inherit;text-decoration:none">
                            "Designbook"
                        </a>
                    </p>
                    <form method="get" action="">
                        <input
                            type="search"
                            name="q"
                            value=(filter)
                            placeholder="Search stories"
                            onchange="this.form.submit()"
                            style="width:100%;box-sizing:border-box;padding:6px 10px;font-size:13px;border:1px solid #e5e0d8;border-radius:8px;background:#fff"
                        >
                        for pair in toolbar.hidden() {
                            <input type="hidden" name=(pair.0) value=(pair.1)>
                        }
                    </form>
                    for group in &tree {
                        <details
                            open=(group_open(group, current, filtering))
                            style="margin-top:8px"
                        >
                            <summary
                                style="font-size:12px;font-weight:700;color:#8a7a6d;cursor:pointer;list-style:none"
                            >
                                (group.name)
                            </summary>
                            for component in &group.components {
                                <p style="margin:8px 0 2px;font-size:13px;font-weight:600">
                                    (component.name)
                                    " "
                                    <span style="color:#8a7a6d;font-weight:400">
                                        "("
                                        (component.stories.len())
                                        ")"
                                    </span>
                                </p>
                                <ul style="margin:0;padding:0;list-style:none">
                                    for name in &component.stories {
                                        let url = format!(
                                            "{}{}",
                                            story_url(group.name, component.name, name),
                                            suffix,
                                        );
                                        let active = url_story_part(&url) == current;
                                        <li>
                                            <a
                                                href=(url)
                                                aria-current=(active.then_some("true"))
                                                style=(if active {
                                                    "display:block;padding:2px 8px;font-size:13px;color:#2c221e;text-decoration:none;border-radius:6px;background:#e8e0d4;font-weight:700"
                                                } else {
                                                    "display:block;padding:2px 8px;font-size:13px;color:#2c221e;text-decoration:none;border-radius:6px"
                                                })
                                            >
                                                (name)
                                            </a>
                                        </li>
                                    }
                                </ul>
                            }
                        </details>
                    }
                </nav>
                <main style="flex:1;padding:24px;max-width:1100px">(child)</main>
            </body>
        </html>
    })
}

/// Whether a sidebar group starts expanded: groups holding the open
/// story, or every group while a search filter is active.
fn group_open(group: &SidebarGroup, current: &str, filtering: bool) -> bool {
    filtering
        || group.components.iter().any(|component| {
            component
                .stories
                .iter()
                .any(|name| story_url(group.name, component.name, name) == current)
        })
}

/// The story path inside a dashboard link, without its query string.
fn url_story_part(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// The frame's own wrapper, without toolbar overrides.
#[component]
async fn frame_body(frame: Frame, #[default] child: Child<'_>) -> Result<impl View> {
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

/// Frames a story canvas per its frame choice, inside the toolbar's
/// width constraint and backdrop override.
#[component]
async fn framed(
    frame: Frame,
    width: Option<&'static str>,
    backdrop: Option<&'static str>,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div style=(width)>
            match backdrop {
                Some(backdrop) => {
                    <div style=(backdrop)>frame_body(frame: frame, (child))</div>
                }
                None => {
                    frame_body(frame: frame, (child))
                }
            }
        </div>
    })
}

/// The human type name for a control kind, for the controls table.
fn kind_label(kind: ParamKind) -> &'static str {
    match kind {
        ParamKind::Select(_) => "select",
        ParamKind::Toggle => "boolean",
        ParamKind::Text => "string",
    }
}

/// The options for a select control, joined for the controls table.
fn kind_options(kind: ParamKind) -> String {
    match kind {
        ParamKind::Select(options) => options.join(", "),
        ParamKind::Toggle => "true".to_owned(),
        ParamKind::Text => "free text".to_owned(),
    }
}

/// One live control: the signal behind a query key.
#[derive(Debug, Clone)]
enum ControlSignal {
    Text(Signal<String>),
    Toggle(Signal<bool>),
}

/// The live region: toolbar, canvas, and controls for one story.
///
/// Control inputs write per-control signals, re-rendering only this
/// shard. The story itself renders eagerly to a string inside the
/// shard body, where the joined control query is visible through
/// [`StoryQuery`]; stories with live regions show their first content
/// here. A static `syncStoryUrl` call keeps the address bar on the
/// current values; without JavaScript the form submits normally.
#[shard]
async fn canvas(
    cx: &Cx,
    group: String,
    component: String,
    name: String,
    query: String,
) -> Result<impl View> {
    struct Live {
        entry: &'static StoryEntry,
        viewport: Signal<String>,
        bg: Signal<String>,
        states: Vec<ControlSignal>,
        viewport_preset: Viewport,
        bg_preset: Background,
        reset_url: String,
        rendered: String,
    }

    let live = match find(&group, &component, &name) {
        None => None,
        Some(entry) => {
            let viewport = signal(&cx.keyed("toolbar:viewport"), || {
                query_value(Some(&query), "viewport")
                    .unwrap_or("full")
                    .to_owned()
            });
            let bg = signal(&cx.keyed("toolbar:bg"), || {
                query_value(Some(&query), "bg")
                    .unwrap_or("default")
                    .to_owned()
            });

            let mut states = Vec::new();
            for param in entry.meta.params {
                let keyed = cx.keyed(format!("control:{}", param.name));
                match param.kind {
                    ParamKind::Select(_) | ParamKind::Text => {
                        let initial = query_value(Some(&query), param.name)
                            .unwrap_or("")
                            .to_owned();
                        states.push(ControlSignal::Text(signal(&keyed, || initial)));
                    }
                    ParamKind::Toggle => {
                        let initial = query_value(Some(&query), param.name) == Some("true");
                        states.push(ControlSignal::Toggle(signal(&keyed, || initial)));
                    }
                }
            }

            let viewport_preset = Viewport::parse(Some(&viewport.get()));
            let bg_preset = Background::parse(Some(&bg.get()));

            let mut pairs = Vec::new();
            for (param, state) in entry.meta.params.iter().zip(&states) {
                match state {
                    ControlSignal::Text(value) => {
                        let value = value.get();
                        if !value.is_empty() {
                            pairs.push((param.name.to_owned(), value));
                        }
                    }
                    ControlSignal::Toggle(value) => {
                        if value.get() {
                            pairs.push((param.name.to_owned(), "true".to_owned()));
                        }
                    }
                }
            }
            let effective = serde_urlencoded::to_string(&pairs).unwrap_or_default();
            let reset_url = format!(
                "{}{}",
                story_url(entry.meta.group, entry.meta.component, entry.meta.name),
                Toolbar {
                    viewport: viewport_preset,
                    bg: bg_preset,
                }
                .suffix()
            );

            let rendered = {
                let scoped = cx.with(StoryQuery(effective));
                (entry.render)(&scoped).first().await?.render(&scoped)
            };

            Some(Live {
                entry,
                viewport,
                bg,
                states,
                viewport_preset,
                bg_preset,
                reset_url,
                rendered,
            })
        }
    };
    Ok(view! {
        cx =>
        match live {
            Some(
                Live {
                    entry,
                    viewport,
                    bg,
                    states,
                    viewport_preset,
                    bg_preset,
                    reset_url,
                    rendered,
                },
            ) => {
                <form method="get" action="" onsubmit="return false">
                    <div
                        style="display:flex;gap:16px;align-items:flex-end;margin-bottom:16px;flex-wrap:wrap"
                    >
                        <div>
                            <div
                                style="font-size:12px;font-weight:700;color:#8a7a6d;margin-bottom:4px"
                            >
                                "Viewport"
                            </div>
                            <select
                                name="viewport"
                                onchange="syncStoryUrl(this.form)"
                                style="padding:4px 8px;font-size:13px;border:1px solid #e5e0d8;border-radius:6px;background:#fff"
                                @change=$(|event: Event| viewport.set(event.target.value))
                            >
                                <option value="full" selected=(viewport.get() == "full")>
                                    "Full width"
                                </option>
                                <option
                                    value="desktop"
                                    selected=(viewport.get() == "desktop")
                                >
                                    "Desktop 1024"
                                </option>
                                <option
                                    value="tablet"
                                    selected=(viewport.get() == "tablet")
                                >
                                    "Tablet 768"
                                </option>
                                <option
                                    value="mobile"
                                    selected=(viewport.get() == "mobile")
                                >
                                    "Mobile 375"
                                </option>
                            </select>
                        </div>
                        <div>
                            <div
                                style="font-size:12px;font-weight:700;color:#8a7a6d;margin-bottom:4px"
                            >
                                "Backdrop"
                            </div>
                            <select
                                name="bg"
                                onchange="syncStoryUrl(this.form)"
                                style="padding:4px 8px;font-size:13px;border:1px solid #e5e0d8;border-radius:6px;background:#fff"
                                @change=$(|event: Event| bg.set(event.target.value))
                            >
                                <option value="default" selected=(bg.get() == "default")>
                                    "Frame default"
                                </option>
                                <option value="paper" selected=(bg.get() == "paper")>
                                    "Paper"
                                </option>
                                <option value="pine" selected=(bg.get() == "pine")>
                                    "Pine"
                                </option>
                                <option value="dark" selected=(bg.get() == "dark")>
                                    "Dark"
                                </option>
                            </select>
                        </div>
                        <a
                            href=(reset_url)
                            style="font-size:13px;font-weight:600;color:#8a7a6d"
                        >
                            "Reset controls"
                        </a>
                        <button
                            type="button"
                            onclick="navigator.clipboard.writeText(window.location.href)"
                            style="padding:6px 12px;font-size:13px;font-weight:600;border:1px solid #e5e0d8;border-radius:6px;background:#fff;cursor:pointer"
                        >
                            "Copy link"
                        </button>
                    </div>

                    framed(
                        frame: entry.meta.frame,
                        width: viewport_preset.wrap(),
                        backdrop: bg_preset.backdrop(),
                        (Unescaped::new_unchecked(rendered))
                    )

                    if !entry.meta.params.is_empty() {
                        <section style="margin-top:24px">
                            <h2>"Controls"</h2>
                            <table
                                style="border-collapse:collapse;margin-bottom:16px;font-size:13px"
                            >
                                <tr style="text-align:left;color:#8a7a6d">
                                    <th style="padding:4px 12px 4px 0;font-weight:600">
                                        "Control"
                                    </th>
                                    <th style="padding:4px 12px 4px 0;font-weight:600">
                                        "Type"
                                    </th>
                                    <th style="padding:4px 0;font-weight:600">"Values"</th>
                                </tr>
                                for param in entry.meta.params {
                                    <tr style="border-top:1px solid #e5e0d8">
                                        <td style="padding:4px 12px 4px 0;font-weight:600">
                                            (param.name)
                                        </td>
                                        <td style="padding:4px 12px 4px 0;color:#8a7a6d">
                                            (kind_label(param.kind))
                                        </td>
                                        <td style="padding:4px 0;color:#8a7a6d">
                                            (kind_options(param.kind))
                                        </td>
                                    </tr>
                                }
                            </table>
                            for (param, state) in entry.meta.params.iter().zip(states) {
                                match state {
                                    ControlSignal::Text(handle) => {
                                        match param.kind {
                                            ParamKind::Select(options) => {
                                                <div
                                                    style="display:flex;align-items:center;gap:8px;margin-bottom:8px"
                                                >
                                                    <span style="width:140px;font-size:13px;font-weight:600">
                                                        (param.name)
                                                    </span>
                                                    <select
                                                        name=(param.name)
                                                        onchange="syncStoryUrl(this.form)"
                                                        style="flex:1;padding:4px 8px;font-size:13px;border:1px solid #e5e0d8;border-radius:6px;background:#fff"
                                                        @change=$(|event: Event| handle.set(event.target.value))
                                                    >
                                                        for option in options {
                                                            <option
                                                                value=(option)
                                                                selected=(handle.get().as_str() == *option)
                                                            >
                                                                (option)
                                                            </option>
                                                        }
                                                    </select>
                                                </div>
                                            }
                                            _ => {
                                                <div
                                                    style="display:flex;align-items:center;gap:8px;margin-bottom:8px"
                                                >
                                                    <span style="width:140px;font-size:13px;font-weight:600">
                                                        (param.name)
                                                    </span>
                                                    <input
                                                        type="text"
                                                        name=(param.name)
                                                        value=(handle.get())
                                                        onchange="syncStoryUrl(this.form)"
                                                        style="flex:1;padding:4px 8px;font-size:13px;border:1px solid #e5e0d8;border-radius:6px"
                                                        @change=$(|event: Event| handle.set(event.target.value))
                                                    >
                                                </div>
                                            }
                                        }
                                    }
                                    ControlSignal::Toggle(handle) => {
                                        <div
                                            style="display:flex;align-items:center;gap:8px;margin-bottom:8px"
                                        >
                                            <span style="width:140px;font-size:13px;font-weight:600">
                                                (param.name)
                                            </span>
                                            <input
                                                type="checkbox"
                                                name=(param.name)
                                                value="true"
                                                checked=(handle.get())
                                                onchange="syncStoryUrl(this.form)"
                                                @change=$(|event: Event| handle.set(event.target.checked))
                                            >
                                        </div>
                                    }
                                }
                            }
                            <button
                                type="submit"
                                style="margin-top:8px;padding:6px 16px;font-size:13px;font-weight:700;border:1px solid #e5e0d8;border-radius:6px;background:#fff;cursor:pointer"
                            >
                                "Apply"
                            </button>
                        </section>
                    }
                </form>
            }
            None => {
                <p>"Unknown story."</p>
            }
        }
    })
}

/// The dashboard index.
fn index_view(cx: &Cx) -> impl View {
    let empty = stories().is_empty();
    let query = uri(cx).query();
    let toolbar = Toolbar::parse(query);
    let filter = query_value(query, "q").map(str::to_owned);
    view! {
        cx =>
        ((header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        shell(
            title: "Index",
            current: "/",
            toolbar: toolbar,
            filter: filter,
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
    let toolbar = Toolbar::parse(query.as_deref());
    let filter = query_value(query.as_deref(), "q").map(str::to_owned);
    let canonical = story_url(group, component, name);

    Ok(view! {
        cx =>
        ((header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        shell(
            title: entry.meta.name,
            current: canonical.as_str(),
            toolbar: toolbar,
            filter: filter,
            <h1 style="margin-top:0">(entry.meta.name)</h1>
            <p style="color:#8a7a6d">
                (entry.meta.group)
                " / "
                (entry.meta.component)
            </p>
            if !entry.meta.description.is_empty() {
                <p>(entry.meta.description)</p>
            }

            canvas(
                group: group.to_owned(),
                component: component.to_owned(),
                name: name.to_owned(),
                query: query.unwrap_or_default()
            )
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
            .route(canvas)
    }
}

#[cfg(test)]
mod tests {
    use topcoat_view::ViewExt;
    use topcoat_view_macro::view;

    use super::*;
    use crate::{ParamDef, StoryMeta, controls};

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

    /// A probe story rendering its control query, submitted straight to
    /// the registry without the macro.
    fn probe_render(cx: &Cx) -> BoxView<'_> {
        #[derive(Debug, Default, serde::Deserialize)]
        struct ProbeQuery {
            tone: Option<String>,
        }
        let tone = controls::<ProbeQuery>(cx)
            .tone
            .unwrap_or_else(|| "plain".to_owned());
        view! {
            cx =>
            <span>
                "probe:"
                (tone)
            </span>
        }
        .boxed()
    }

    inventory::submit! {
        StoryEntry {
            meta: StoryMeta {
                group: "Probe",
                component: "Canvas",
                name: "Query",
                description: "",
                frame: Frame::Padded,
                source_file: file!(),
                params: &[ParamDef {
                    name: "tone",
                    kind: ParamKind::Text,
                }],
            },
            render: probe_render,
        }
    }

    #[tokio::test]
    async fn shard_applies_control_query() {
        let cx = Cx::default();
        let html = view! {
            cx =>
            canvas(
                group: "Probe".to_owned(),
                component: "Canvas".to_owned(),
                name: "Query".to_owned(),
                query: "tone=loud".to_owned()
            )
        }
        .first()
        .await
        .expect("shard renders")
        .render(&cx);
        assert!(html.contains("probe:loud"), "{html}");
    }

    #[test]
    fn toolbar_parses_canvas_presets_leniently() {
        assert_eq!(Toolbar::parse(None), Toolbar::default());
        assert_eq!(
            Toolbar::parse(Some("viewport=mobile&bg=pine")),
            Toolbar {
                viewport: Viewport::Mobile,
                bg: Background::Pine,
            }
        );
        assert_eq!(
            Toolbar::parse(Some("viewport=wide&bg=neon")),
            Toolbar::default()
        );
    }

    #[test]
    fn toolbar_suffix_preserves_only_set_presets() {
        assert_eq!(Toolbar::default().suffix(), "");
        assert_eq!(
            Toolbar {
                viewport: Viewport::Tablet,
                bg: Background::Pine,
            }
            .suffix(),
            "?viewport=tablet&bg=pine"
        );
    }

    #[test]
    fn sidebar_filter_keeps_matching_branches_only() {
        let all = sidebar_tree(None);
        assert!(all.iter().any(|group| group.name == "Zebra"));
        let filtered = sidebar_tree(Some("zeb"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Zebra");
        assert!(sidebar_tree(Some("nothing matches this")).is_empty());
    }
}
