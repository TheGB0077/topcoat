#![cfg(feature = "router")]

use topcoat::{
    Result,
    router::{Body, Router, request::Request, to_bytes},
    story::{RouterBuilderDashboardExt, story},
    view::{View, component, view},
};

/// A plain card.
#[story(
    group = "Shop",
    component = "Badge",
    name = "Default",
    params = [select("tone", ["plain", "loud"]), toggle("glow")],
    fixtures = [tone = "plain".to_owned()],
)]
#[component]
async fn badge_default(tone: String, glow: bool) -> Result<impl View> {
    Ok(view! {
        <span>
            "badge:"
            (tone)
            " "
            (glow)
        </span>
    })
}

/// A greeting in the toolbar locale.
#[story(
    group = "Shop",
    component = "Badge",
    name = "Localized",
    params = [global("locale", ["en", "pt"])],
    fixtures = [locale = "en".to_owned()],
)]
#[component]
async fn badge_localized(locale: String) -> Result<impl View> {
    Ok(view! {
        <span>
            "hello "
            (locale)
        </span>
    })
}

/// Sends a `GET` request to `router` and collects the status and body.
async fn fetch(router: &Router, path: &str) -> (u16, String) {
    let request = Request::builder().uri(path).body(Body::empty()).unwrap();
    let response = router.handle(request).await;
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

fn router() -> Router {
    Router::builder().dashboard().build()
}

#[tokio::test]
async fn index_lists_registered_stories() {
    let (status, html) = fetch(&router(), "/").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("/Shop/Badge/Default"), "{html}");
}

#[tokio::test]
async fn story_page_renders_canvas_controls_and_source() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Default").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("<iframe"), "{html}");
    assert!(html.contains("?embed=1"), "{html}");
    assert!(html.contains("tone"), "{html}");
    assert!(html.contains("glow"), "{html}");
    assert!(html.contains("async fn badge_default"), "{html}");
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?embed=1").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("badge:plain false"), "{html}");
}

#[tokio::test]
async fn story_page_applies_control_values() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?embed=1&tone=loud&glow=true").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("badge:loud true"), "{html}");
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?tone=loud").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("?embed=1&amp;tone=loud"), "{html}");
}

#[tokio::test]
async fn toolbar_presets_constrain_canvas_and_backdrop() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?viewport=mobile&bg=pine").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("max-width:375px"), "{html}");
    assert!(html.contains("background:#1c2b24"), "{html}");
}

#[tokio::test]
async fn sidebar_preserves_toolbar_and_marks_current_story() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?viewport=mobile").await;
    assert_eq!(status, 200, "{html}");
    assert!(
        html.contains("/Shop/Badge/Default?viewport=mobile"),
        "{html}"
    );
    assert!(html.contains("aria-current=\"true\""), "{html}");
}

#[tokio::test]
async fn reset_link_clears_controls_but_keeps_toolbar() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?tone=loud&viewport=mobile").await;
    assert_eq!(status, 200, "{html}");
    assert!(
        html.contains("/Shop/Badge/Default?viewport=mobile\""),
        "{html}"
    );
    assert!(!html.contains("tone=loud&viewport"), "{html}");
}

#[tokio::test]
async fn search_filters_sidebar_to_matching_stories() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?q=zzz-no-match").await;
    assert_eq!(status, 200, "{html}");
    // The source panel echoes this file escaped, so match the raw
    // attribute only a real sidebar link renders.
    assert!(!html.contains("aria-current=\"true\""), "{html}");
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?q=badge").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("aria-current=\"true\""), "{html}");
}

#[tokio::test]
async fn unknown_story_is_not_found() {
    let (status, _) = fetch(&router(), "/Shop/Badge/Missing").await;
    assert_eq!(status, 404);
    let (status, _) = fetch(&router(), "/Shop/Badge/Missing?embed=1").await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn embed_page_renders_story_without_dashboard_chrome() {
    let (status, html) = fetch(
        &router(),
        "/Shop/Badge/Default?embed=1&viewport=mobile&bg=pine",
    )
    .await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("badge:plain false"), "{html}");
    assert!(html.contains("width=device-width"), "{html}");
    assert!(!html.contains("Search stories"), "{html}");
}

#[tokio::test]
async fn canvas_posts_controls_into_isolated_frame() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?tone=loud").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("id=\"story-canvas\""), "{html}");
    assert!(html.contains("postStoryControl"), "{html}");
    assert!(html.contains("data-story-post"), "{html}");
    assert!(!html.contains("<span>badge:"), "{html}");
}

#[tokio::test]
async fn embed_page_bridges_hidden_controls_live() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?embed=1&tone=loud&glow=true").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("badge:loud true"), "{html}");
    assert!(html.contains("data-story-control=\"tone\""), "{html}");
    assert!(html.contains("data-story-control=\"glow\""), "{html}");
    assert!(html.contains("storyControl"), "{html}");
}

#[tokio::test]
async fn manifest_lists_story_urls() {
    let (status, html) = fetch(&router(), "/manifest.json").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("/Shop/Badge/Default"), "{html}");
    assert!(html.contains("?embed=1"), "{html}");
}

#[tokio::test]
async fn global_toolbar_switches_declared_locales() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Localized").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("name=\"locale\""), "{html}");
    let (status, html) = fetch(&router(), "/Shop/Badge/Localized?embed=1").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains(">hello en<"), "{html}");
    let (status, html) = fetch(&router(), "/Shop/Badge/Localized?embed=1&locale=pt").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains(">hello pt<"), "{html}");
    assert!(!html.contains(">hello en<"), "{html}");
}

#[tokio::test]
async fn navigation_preserves_global_values() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Localized?locale=pt").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("/Shop/Badge/Default?locale=pt"), "{html}");
    assert!(html.contains("/Shop/Badge/Localized?locale=pt\""), "{html}");
}
