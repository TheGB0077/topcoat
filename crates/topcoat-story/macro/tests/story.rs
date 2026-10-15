use topcoat::{
    Result,
    story::{Frame, ParamKind, stories, story},
    view::{View, component, view},
};

/// A plain card.
#[story(
    group = "Shop",
    component = "Badge",
    name = "Default",
    params = [select("tone", ["plain", "loud"]), toggle("glow")],
)]
#[component]
async fn badge_default() -> Result<impl View> {
    Ok(view! { <span>"new"</span> })
}

#[test]
fn story_registers_in_stories() {
    let entry = stories()
        .iter()
        .find(|entry| entry.meta.name == "Default")
        .expect("the story is registered");
    assert_eq!(entry.meta.group, "Shop");
    assert_eq!(entry.meta.component, "Badge");
    assert_eq!(entry.meta.description, "A plain card.");
    assert_eq!(entry.meta.frame, Frame::Padded);
    assert!(entry.meta.source_file.ends_with("tests/story.rs"));
    assert_eq!(entry.meta.params.len(), 2);
    assert_eq!(entry.meta.params[0].name, "tone");
    assert_eq!(
        entry.meta.params[0].kind,
        ParamKind::Select(&["plain", "loud"])
    );
    assert_eq!(entry.meta.params[1].name, "glow");
    assert_eq!(entry.meta.params[1].kind, ParamKind::Toggle);
}

/// A card labeled with its deal title.
#[story(
    group = "Shop",
    component = "Badge",
    name = "Labeled",
    title = "clearance"
)]
#[component]
async fn badge_labeled(title: &str) -> Result<impl View> {
    Ok(view! { <span>(title)</span> })
}

#[test]
fn story_with_fixture_registers() {
    let entry = stories()
        .iter()
        .find(|entry| entry.meta.name == "Labeled")
        .expect("the story is registered");
    assert_eq!(entry.meta.component, "Badge");
}

#[tokio::test]
async fn story_with_fixture_renders_its_prop() {
    use topcoat::{context::Cx, view::ViewExt};

    let entry = stories()
        .iter()
        .find(|entry| entry.meta.name == "Labeled")
        .expect("the story is registered");
    let cx = Cx::default();
    let html = (entry.render)(&cx)
        .single()
        .await
        .expect("story renders")
        .render(&cx);
    assert!(html.contains("clearance"), "{html}");
}
