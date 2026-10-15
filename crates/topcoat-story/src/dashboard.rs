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

use super::{Frame, ParamKind, StoryEntry, find, query_value, stories};

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

    /// The canvas iframe's own style: full width up to the preset, so
    /// viewport media queries inside see a device-width viewport.
    /// The fixed height is corrected to the content height on load.
    fn iframe_style(self) -> &'static str {
        match self {
            Self::Full => "width:100%;height:600px;border:0;display:block",
            Self::Desktop => {
                "width:100%;max-width:1024px;height:600px;margin:0 auto;border:0;display:block"
            }
            Self::Tablet => {
                "width:100%;max-width:768px;height:600px;margin:0 auto;border:0;display:block"
            }
            Self::Mobile => {
                "width:100%;max-width:375px;height:600px;margin:0 auto;border:0;display:block"
            }
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
/// string belongs to the story's controls, globals, or the sidebar search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Toolbar {
    viewport: Viewport,
    bg: Background,
}

impl Toolbar {
    /// Reads the toolbar keys out of a raw query string.
    fn parse(query: Option<&str>) -> Self {
        Self {
            viewport: Viewport::parse(query_value(query, "viewport").as_deref()),
            bg: Background::parse(query_value(query, "bg").as_deref()),
        }
    }
}

/// Query pairs that ride along dashboard navigation: canvas presets,
/// every global value present, and the sidebar search. Story controls
/// never ride: each story owns its knobs.
fn preserved(query: Option<&str>) -> Vec<(String, String)> {
    let toolbar = Toolbar::parse(query);
    let mut pairs: Vec<(String, String)> = toolbar
        .viewport
        .query()
        .into_iter()
        .chain(toolbar.bg.query())
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    for (key, _) in supported_globals() {
        if let Some(value) = query_value(query, key) {
            pairs.push((key.to_owned(), value));
        }
    }
    if let Some(value) = query_value(query, "q") {
        pairs.push(("q".to_owned(), value));
    }
    pairs
}

/// The query suffix preserving navigation state, for links that
/// navigate without submitting a form.
fn preserved_suffix(query: Option<&str>) -> String {
    let mut suffix = String::new();
    for (key, value) in preserved(query) {
        suffix.push(if suffix.is_empty() { '?' } else { '&' });
        suffix.push_str(&encode_segment(&key));
        suffix.push('=');
        suffix.push_str(&encode_segment(&value));
    }
    suffix
}

/// Preserved pairs minus the given keys, for hidden form inputs: keys
/// a form already submits need no duplication.
fn carry(query: Option<&str>, skip: &[&str]) -> Vec<(String, String)> {
    preserved(query)
        .into_iter()
        .filter(|(key, _)| !skip.contains(&key.as_str()))
        .collect()
}

/// Every global the registry declares: the union of `global` controls
/// across stories, keyed by name with merged options in first-seen
/// order. Globals behave like selects within their own story and ride
/// along every dashboard navigation; the toolbar below offers one
/// switcher per key. Declare the same key with the same options in
/// each participating story.
fn supported_globals() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut globals: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    for entry in stories() {
        for param in entry.meta.params {
            let ParamKind::Global(options) = param.kind else {
                continue;
            };
            let global =
                if let Some(global) = globals.iter_mut().find(|global| global.0 == param.name) {
                    global
                } else {
                    globals.push((param.name, Vec::new()));
                    globals.last_mut().expect("global was pushed")
                };
            for option in options {
                if !global.1.contains(option) {
                    global.1.push(option);
                }
            }
        }
    }
    globals
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

/// Whether a bundled filename is the generated Tailwind stylesheet.
///
/// Tailwind's build writes `tailwind.css` by default, which the bundler
/// stores as `tailwind-<hash>.css`. Custom output names are not found.
fn is_tailwind_stylesheet(name: &str) -> bool {
    let stem = name.starts_with("tailwind");
    let css = Path::new(name).extension().is_some_and(|ext| ext == "css");
    stem && css
}

/// The application's Tailwind stylesheet URL, when the app bundles one.
///
/// Stories render inside the dashboard shell, so the shell links the
/// app's own stylesheet and story markup gets production styling. The
/// dashboard chrome itself keeps inline styles.
fn tailwind_stylesheet(config: &AssetConfig) -> Option<String> {
    let stylesheet = config
        .catalog()
        .assets()
        .find(|asset| is_tailwind_stylesheet(asset.name()))?;
    Some(format!(
        "{}/{}",
        config.base_url(),
        stylesheet.encoded_name()
    ))
}

/// Keeps the address bar on the live form's values without navigating.
/// Control inputs call it on change; the shard re-render answers the
/// state change, so this only records it for sharing.
const SYNC_SCRIPT: &str = "function syncStoryUrl(form){var params=new URLSearchParams(new FormData(form));var query=params.toString();history.replaceState(null,'',location.pathname+(query?'?'+query:''));}";

/// The dashboard shell: sidebar registry tree plus the page content.
/// `current` is the request path, highlighting the open story. `query`
/// is the raw query string: toolbar presets, globals, and the search
/// filter all ride along navigation from it, and the tree narrows to
/// matching names.
#[component]
async fn shell(
    cx: &Cx,
    title: &str,
    current: &str,
    query: Option<String>,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let filter = query_value(query.as_deref(), "q");
    let tree = sidebar_tree(filter.as_deref());
    let suffix = preserved_suffix(query.as_deref());
    let filtering = filter.as_deref().is_some_and(|filter| !filter.is_empty());
    let assets = try_app_context::<AssetConfig>(cx);
    let live = assets.is_some();
    let stylesheet = assets.and_then(tailwind_stylesheet);
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
                if let Some(href) = stylesheet {
                    <link rel="stylesheet" href=(href)>
                }
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
                        for (key, value) in preserved(query.as_deref())
                            .into_iter()
                            .filter(|(key, _)| key != "q") {
                            <input type="hidden" name=(key) value=(value)>
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

/// The frame's own wrapper, without toolbar overrides. The isolated
/// canvas is already at the story's width, so bleed and bare both
/// render edge to edge.
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
            Frame::Dark => {
                <div style="background:#1c2b24;border-radius:12px;padding:24px">
                    (child)
                </div>
            }
            Frame::Bare | Frame::FullBleed => {
                <div>(child)</div>
            }
        }
    })
}

/// The human type name for a control kind, for the controls table.
fn kind_label(kind: ParamKind) -> &'static str {
    match kind {
        ParamKind::Select(_) => "select",
        ParamKind::Global(_) => "global",
        ParamKind::Toggle => "boolean",
        ParamKind::Text => "string",
    }
}

/// The options for a select control, joined for the controls table.
fn kind_options(kind: ParamKind) -> String {
    match kind {
        ParamKind::Select(options) | ParamKind::Global(options) => options.join(", "),
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
/// shard. The story itself renders isolated in an iframe at the
/// viewport preset's width, so viewport media queries see a
/// device-width viewport instead of a narrowed dashboard column; knob
/// changes reload the frame through its live `src`. A static
/// `syncStoryUrl` call keeps the address bar on the current values;
/// without JavaScript the form submits normally.
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
        embed_src: String,
        globals_carry: Vec<(String, String)>,
        carry_over: Vec<(String, String)>,
    }

    let live = match find(&group, &component, &name) {
        None => None,
        Some(entry) => {
            let viewport = signal(&cx.keyed("toolbar:viewport"), || {
                query_value(Some(&query), "viewport").unwrap_or_else(|| "full".to_owned())
            });
            let bg = signal(&cx.keyed("toolbar:bg"), || {
                query_value(Some(&query), "bg").unwrap_or_else(|| "default".to_owned())
            });

            let mut states = Vec::new();
            for param in entry.meta.params {
                let keyed = cx.keyed(format!("control:{}", param.name));
                match param.kind {
                    ParamKind::Select(_) | ParamKind::Global(_) | ParamKind::Text => {
                        let initial = query_value(Some(&query), param.name).unwrap_or_default();
                        states.push(ControlSignal::Text(signal(&keyed, || initial)));
                    }
                    ParamKind::Toggle => {
                        let initial =
                            query_value(Some(&query), param.name).as_deref() == Some("true");
                        states.push(ControlSignal::Toggle(signal(&keyed, || initial)));
                    }
                }
            }

            let viewport_preset = Viewport::parse(Some(viewport.get()).as_deref());
            let bg_preset = Background::parse(Some(bg.get()).as_deref());

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
            let reset_url = format!(
                "{}{}",
                story_url(entry.meta.group, entry.meta.component, entry.meta.name),
                preserved_suffix(Some(&query)),
            );

            let global_keys: Vec<&str> =
                supported_globals().iter().map(|global| global.0).collect();
            let globals_carry = carry(Some(&query), &global_keys);
            let mut live_skip = vec!["viewport", "bg"];
            live_skip.extend(entry.meta.params.iter().map(|param| param.name));
            let carry_over = carry(Some(&query), &live_skip);

            let mut embed_src = format!(
                "{}?embed=1",
                story_url(entry.meta.group, entry.meta.component, entry.meta.name),
            );
            let mut state = Vec::new();
            state.extend(
                viewport_preset
                    .query()
                    .map(|(key, value)| (key.to_owned(), value.to_owned())),
            );
            state.extend(
                bg_preset
                    .query()
                    .map(|(key, value)| (key.to_owned(), value.to_owned())),
            );
            state.extend(pairs.iter().cloned());
            state.extend(carry_over.iter().filter(|(key, _)| key != "q").cloned());
            for (key, value) in &state {
                embed_src.push('&');
                embed_src.push_str(&encode_segment(key));
                embed_src.push('=');
                embed_src.push_str(&encode_segment(value));
            }

            Some(Live {
                entry,
                viewport,
                bg,
                states,
                viewport_preset,
                bg_preset,
                reset_url,
                embed_src,
                globals_carry,
                carry_over,
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
                    embed_src,
                    globals_carry,
                    carry_over,
                },
            ) => {
                if !supported_globals().is_empty() {
                    <form
                        method="get"
                        action=""
                        style="display:flex;gap:16px;align-items:flex-end;margin-bottom:16px;flex-wrap:wrap"
                    >
                        for (key, options) in supported_globals() {
                            <div>
                                <div
                                    style="font-size:12px;font-weight:700;color:#8a7a6d;margin-bottom:4px"
                                >
                                    (key)
                                </div>
                                <select
                                    name=(key)
                                    onchange="this.form.submit()"
                                    style="padding:4px 8px;font-size:13px;border:1px solid #e5e0d8;border-radius:6px;background:#fff"
                                >
                                    for option in options {
                                        <option
                                            value=(option)
                                            selected=(query_value(Some(&query), key).as_deref()
                                                == Some(option))
                                        >
                                            (option)
                                        </option>
                                    }
                                </select>
                            </div>
                        }
                        for (key, value) in globals_carry {
                            <input type="hidden" name=(key) value=(value)>
                        }
                    </form>
                }
                <form method="get" action="" onsubmit="return false">
                    for (key, value) in carry_over {
                        <input type="hidden" name=(key) value=(value)>
                    }
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
                        <a
                            href=(embed_src.clone())
                            target="_blank"
                            style="font-size:13px;font-weight:600;color:#8a7a6d"
                        >
                            "Open isolated"
                        </a>
                    </div>

                    <div style=(viewport_preset.wrap())>
                        <div style=(bg_preset.backdrop())>
                            <iframe
                                title=(entry.meta.name)
                                src=(embed_src)
                                style=(viewport_preset.iframe_style())
                                onload="this.style.height=this.contentDocument.body.scrollHeight+'px'"
                            ></iframe>
                        </div>
                    </div>

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
                                            ParamKind::Select(options) | ParamKind::Global(options) => {
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
    let query = uri(cx).query().map(str::to_owned);
    view! {
        cx =>
        ((header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        shell(
            title: "Index",
            current: "/",
            query: query,
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
    let canonical = story_url(group, component, name);

    Ok(view! {
        cx =>
        ((header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        shell(
            title: entry.meta.name,
            current: canonical.as_str(),
            query: query.clone(),
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

/// One story rendered standalone for the canvas iframe and real-device
/// testing: the story in its frame, without dashboard chrome. Toolbar
/// presets stay dashboard canvas chrome, so only control and global
/// values cross over, read from the same query keys.
async fn embed_view(cx: &Cx) -> Result<impl View> {
    let group = path_param::<Group>(cx);
    let component = path_param::<Component>(cx);
    let name = path_param::<StoryName>(cx);
    let entry: &'static StoryEntry = find(group, component, name).ok_or_not_found()?;
    let query = uri(cx).query().unwrap_or_default();
    let mut pairs = Vec::new();
    for param in entry.meta.params {
        match param.kind {
            ParamKind::Select(_) | ParamKind::Global(_) | ParamKind::Text => {
                if let Some(value) = query_value(Some(query), param.name)
                    && !value.is_empty()
                {
                    pairs.push((param.name.to_owned(), value));
                }
            }
            ParamKind::Toggle => {
                if query_value(Some(query), param.name).as_deref() == Some("true") {
                    pairs.push((param.name.to_owned(), "true".to_owned()));
                }
            }
        }
    }
    let effective = serde_urlencoded::to_string(&pairs).unwrap_or_default();
    let rendered = (entry.render)(cx, effective).await?;
    let assets = try_app_context::<AssetConfig>(cx);
    let stylesheet = assets.and_then(tailwind_stylesheet);
    let live = assets.is_some();
    Ok(view! {
        cx =>
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>(entry.meta.name)</title>
                if let Some(src) = dev_script() {
                    <script src=(src)></script>
                }
                if let Some(href) = stylesheet {
                    <link rel="stylesheet" href=(href)>
                }
                if live {
                    <script
                        type="module"
                        src=(topcoat_runtime::SCRIPT)
                        data-topcoat-usize-bits=(usize::BITS)
                    ></script>
                }
            </head>
            <body style="margin:0">
                frame_body(frame: entry.meta.frame, (rendered))
            </body>
        </html>
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
    /// The same story without dashboard chrome, for snapshot and pixel
    /// test runners.
    pub embed_url: String,
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
                embed_url: format!(
                    "{}?embed=1",
                    story_url(entry.meta.group, entry.meta.component, entry.meta.name),
                ),
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

/// `GET /{group}/{component}/{story}` — one story, or its standalone
/// rendering with `?embed=1`.
fn story_page_route(cx: &Cx, _body: Body) -> BoxView<'_> {
    page(async move {
        let embed = uri(cx)
            .query()
            .is_some_and(|query| query_value(Some(query), "embed").is_some());
        if embed {
            Ok(embed_view(cx).await?.boxed())
        } else {
            Ok(story_page_view(cx)?.boxed())
        }
    })
}

/// Registers the story dashboard's routes on a router builder.
///
/// The routes sit at the root of the router they are registered on: the
/// index at `/`, one page per story at `/{group}/{component}/{story}`,
/// and the story manifest at `/manifest.json`. Each story page renders
/// standalone with `?embed=1`, which is also the canvas iframe's source.
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
    use crate::{ParamDef, StoryFuture, StoryMeta};

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

    /// A probe story rendering static content, submitted straight to
    /// the registry without the macro. Exercises shard-inline rendering
    /// with no router involved.
    fn probe_render(cx: &Cx, _query: String) -> StoryFuture<'_> {
        Box::pin(async move { Ok(view! { cx => <span>"probe"</span> }.boxed()) })
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
    async fn shard_points_canvas_at_isolated_story() {
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
        assert!(html.contains("<iframe"), "{html}");
        assert!(
            html.contains("/Probe/Canvas/Query?embed=1&amp;tone=loud"),
            "{html}"
        );
    }

    #[test]
    fn iframe_styles_constrain_width_per_preset() {
        assert!(!Viewport::Full.iframe_style().contains("max-width"));
        assert!(Viewport::Full.iframe_style().starts_with("width:100%"));
        assert!(
            Viewport::Desktop
                .iframe_style()
                .contains("max-width:1024px")
        );
        assert!(Viewport::Tablet.iframe_style().contains("max-width:768px"));
        assert!(Viewport::Mobile.iframe_style().contains("max-width:375px"));
    }

    inventory::submit! {
        StoryEntry {
            meta: StoryMeta {
                group: "I18n",
                component: "Greeting",
                name: "Localized",
                description: "",
                frame: Frame::Padded,
                source_file: file!(),
                params: &[
                    ParamDef {
                        name: "locale",
                        kind: ParamKind::Global(&["en", "pt"]),
                    },
                    ParamDef {
                        name: "theme",
                        kind: ParamKind::Global(&["light", "dark"]),
                    },
                ],
            },
            render: probe_render,
        }
    }

    #[test]
    fn supported_globals_unions_declared_keys() {
        assert_eq!(
            supported_globals(),
            vec![
                ("locale", vec!["en", "pt"]),
                ("theme", vec!["light", "dark"]),
            ]
        );
    }

    #[test]
    fn preserved_carries_presets_globals_and_search() {
        assert_eq!(preserved(None), Vec::<(String, String)>::new());
        assert_eq!(
            preserved(Some("viewport=tablet&bg=pine&tone=loud&q=card")),
            vec![
                ("viewport".to_owned(), "tablet".to_owned()),
                ("bg".to_owned(), "pine".to_owned()),
                ("q".to_owned(), "card".to_owned()),
            ]
        );
        assert_eq!(preserved_suffix(None), "");
        assert_eq!(
            preserved_suffix(Some("viewport=mobile")),
            "?viewport=mobile"
        );
    }

    #[test]
    fn carry_skips_form_native_keys() {
        assert_eq!(
            carry(Some("viewport=mobile&locale=pt&tone=loud"), &["tone"]),
            vec![
                ("viewport".to_owned(), "mobile".to_owned()),
                ("locale".to_owned(), "pt".to_owned()),
            ]
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

    #[test]
    fn tailwind_match_keeps_generated_output_only() {
        assert!(is_tailwind_stylesheet("tailwind-a1b2c3d4e5f60718.css"));
        assert!(!is_tailwind_stylesheet("app-a1b2c3d4e5f60718.css"));
        assert!(!is_tailwind_stylesheet("tailwind.config.js"));
        assert!(!is_tailwind_stylesheet("my-tailwind-a1b2c3d4e5f60718.css"));
    }
}
