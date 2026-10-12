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
