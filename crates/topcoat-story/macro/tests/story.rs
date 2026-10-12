use topcoat::{
    Result,
    story::{Frame, stories, story},
    view::{View, component, view},
};

/// A plain card.
#[story(group = "Shop", component = "Badge", name = "Default")]
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
}
