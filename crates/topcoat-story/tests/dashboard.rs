#![cfg(feature = "router")]

use topcoat::{
    Result,
    context::Cx,
    router::{Body, Router, query_params, request::Request, to_bytes},
    story::{RouterBuilderDashboardExt, story},
    view::{View, component, view},
};

#[query_params]
struct BadgeQuery {
    tone: Option<String>,
    glow: Option<bool>,
}

/// A plain card.
#[story(
    group = "Shop",
    component = "Badge",
    name = "Default",
    params = [select("tone", ["plain", "loud"]), toggle("glow")],
)]
#[component]
async fn badge_default(cx: &Cx) -> Result<impl View> {
    let query = query_params::<BadgeQuery>(cx).ok();
    let tone = query
        .and_then(|query| query.tone.clone())
        .unwrap_or_else(|| "plain".to_owned());
    let glow = query.and_then(|query| query.glow).unwrap_or(false);
    Ok(view! {
        <span>
            "badge:"
            (tone)
            " "
            (glow)
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
    assert!(html.contains("badge:plain false"), "{html}");
    assert!(html.contains("tone"), "{html}");
    assert!(html.contains("glow"), "{html}");
    assert!(html.contains("async fn badge_default"), "{html}");
}

#[tokio::test]
async fn story_page_applies_control_values() {
    let (status, html) = fetch(&router(), "/Shop/Badge/Default?tone=loud&glow=true").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("badge:loud true"), "{html}");
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
}

#[tokio::test]
async fn manifest_lists_story_urls() {
    let (status, html) = fetch(&router(), "/manifest.json").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("/Shop/Badge/Default"), "{html}");
}
