#![cfg(feature = "router")]

use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, Router, RouterBuilderDiscoverExt, module_router, query_params, request::Request,
        to_bytes,
    },
    story::story,
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
    module_router!().discover().build()
}

#[tokio::test]
async fn index_lists_registered_stories() {
    let (status, html) = fetch(&router(), "/designbook").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("/designbook/Shop/Badge/Default"), "{html}");
}

#[tokio::test]
async fn story_page_renders_canvas_controls_and_source() {
    let (status, html) = fetch(&router(), "/designbook/Shop/Badge/Default").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("badge:plain false"), "{html}");
    assert!(html.contains("tone"), "{html}");
    assert!(html.contains("glow"), "{html}");
    assert!(html.contains("async fn badge_default"), "{html}");
}

#[tokio::test]
async fn story_page_applies_control_values() {
    let (status, html) = fetch(
        &router(),
        "/designbook/Shop/Badge/Default?tone=loud&glow=true",
    )
    .await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("badge:loud true"), "{html}");
}

#[tokio::test]
async fn unknown_story_is_not_found() {
    let (status, _) = fetch(&router(), "/designbook/Shop/Badge/Missing").await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn manifest_lists_story_urls() {
    let (status, html) = fetch(&router(), "/designbook/manifest.json").await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains("/designbook/Shop/Badge/Default"), "{html}");
}
