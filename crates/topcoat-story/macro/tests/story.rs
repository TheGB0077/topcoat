use topcoat::{
    Result,
    story::{Frame, ParamKind, stories, story},
    view::{Child, View, component, view},
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
            (tone)
            " "
            (glow)
        </span>
    })
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
    fixtures = [title = "clearance"]
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
    let html = (entry.render)(&cx, String::new())
        .await
        .expect("story renders")
        .single()
        .await
        .expect("story polls")
        .render(&cx);
    assert!(html.contains("clearance"), "{html}");
}

/// A card whose tone knob is wired to its prop.
#[story(
    group = "Shop",
    component = "Badge",
    name = "Toned",
    params = [select("tone", ["plain", "loud"])],
    fixtures = [tone = "plain".to_owned()],
)]
#[component]
async fn badge_toned(tone: String) -> Result<impl View> {
    Ok(view! { <span>(tone)</span> })
}

#[tokio::test]
async fn control_value_overrides_fixture() {
    use topcoat::{context::Cx, view::ViewExt};

    async fn render(query: &str) -> String {
        let entry = stories()
            .iter()
            .find(|entry| entry.meta.name == "Toned")
            .expect("the story is registered");
        let cx = Cx::default();
        (entry.render)(&cx, query.to_owned())
            .await
            .expect("story renders")
            .single()
            .await
            .expect("story polls")
            .render(&cx)
    }

    let html = render("tone=loud").await;
    assert!(html.contains("loud"), "{html}");
    assert!(!html.contains("plain"), "{html}");
    let html = render("").await;
    assert!(html.contains("plain"), "{html}");
}

/// An aligned card with optional props.
#[story(
    group = "Shop",
    component = "Badge",
    name = "Aligned",
    params = [select("align", ["left", "center"]), select("count", ["1", "4"])],
    fixtures = [align = None, count = Some(2)],
)]
#[component]
async fn badge_aligned(align: Option<&str>, count: Option<u32>) -> Result<impl View> {
    Ok(view! {
        <span>
            (align.unwrap_or("left"))
            " "
            (count.unwrap_or(0))
        </span>
    })
}

#[tokio::test]
async fn optional_controls_fall_back_to_fixtures() {
    use topcoat::{context::Cx, view::ViewExt};

    async fn render(query: &str) -> String {
        let entry = stories()
            .iter()
            .find(|entry| entry.meta.name == "Aligned")
            .expect("the story is registered");
        let cx = Cx::default();
        (entry.render)(&cx, query.to_owned())
            .await
            .expect("story renders")
            .single()
            .await
            .expect("story polls")
            .render(&cx)
    }

    let html = render("").await;
    assert!(html.contains("left"), "{html}");
    assert!(html.contains('2'), "{html}");
    let html = render("align=center&count=4").await;
    assert!(html.contains("center"), "{html}");
    assert!(html.contains('4'), "{html}");
    assert!(!html.contains('2'), "{html}");
}

/// A panel taking its title as children. Stories cannot pass markup
/// through the attribute, so the story is a thin wrapper rendering the
/// component with inline children.
#[component]
async fn panel(
    #[default] align: Option<&'static str>,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <section>
            (align.unwrap_or("left"))
            (child)
        </section>
    })
}

/// A panel story with literal children and a knobbed alignment.
#[story(
    group = "Shop",
    component = "Panel",
    name = "AlignedPanel",
    params = [select("align", ["left", "center"])],
    fixtures = [align = "left".to_owned()],
)]
#[component]
async fn panel_aligned(align: String) -> Result<impl View> {
    Ok(view! {
        panel(
            align: if align == "center" { Some("center") } else { None },
            "Sale ends Sunday"
        )
    })
}

#[tokio::test]
async fn wrapper_story_renders_children_and_knobs() {
    use topcoat::{context::Cx, view::ViewExt};

    async fn render(query: &str) -> String {
        let entry = stories()
            .iter()
            .find(|entry| entry.meta.name == "AlignedPanel")
            .expect("the story is registered");
        let cx = Cx::default();
        (entry.render)(&cx, query.to_owned())
            .await
            .expect("story renders")
            .single()
            .await
            .expect("story polls")
            .render(&cx)
    }

    let html = render("").await;
    assert!(html.contains("Sale ends Sunday"), "{html}");
    assert!(html.contains("left"), "{html}");
    let html = render("align=center").await;
    assert!(html.contains("center"), "{html}");
    assert!(!html.contains(">left<"), "{html}");
}
