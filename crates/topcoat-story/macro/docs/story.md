# `#[story]`

Registers a story function in the workshop registry.

```rust
use topcoat::{Result, context::Cx, story::story, view::{View, component, view}};

/// Struck-through base price with badge emphasis.
#[story(
    group = "Catalog",
    component = "DealCard",
    name = "Discounted",
    frame = Padded,
    params = [select("tone", ["plain", "loud"]), toggle("glow")],
    deal = fixtures::deal_card_sale(),
)]
#[component]
async fn deal_card_discounted(cx: &Cx, deal: fixtures::Deal) -> Result<impl View> {
    Ok(view! { <p>(deal.title)</p> })
}
# mod fixtures {
#     pub struct Deal { pub title: String }
#     pub fn deal_card_sale() -> Deal { Deal { title: "Sale".to_owned() } }
# }
```

`group`, `component`, and `name` place the story in the sidebar tree, in
any order. `frame` is one of `Padded`, `Bare`, `Dark`, or `FullBleed`, and
defaults to `Padded`. `params` declares the controls panel: `select` with
its options, `toggle` for on/off, and `text` for free input.

Any other `key = value` pair is a fixture: the value the gallery renders
that component prop with, evaluated fresh on every render. Every prop
without `#[default]` needs one, except `cx`; anything else is rejected,
so a misspelled prop or a missing fixture fails the build instead of the
workshop. `#[default]` props may still take a fixture overriding the
default.

```rust
# use topcoat::{Result, context::Cx, story::controls};
#[derive(Default, serde::Deserialize)]
struct CardQuery {
    tone: Option<String>,
}

# async fn read(cx: &Cx) -> Result<String> {
let tone = controls::<CardQuery>(cx).tone.unwrap_or_else(|| "plain".to_owned());
# Ok(tone) }
```

The dashboard sets the query from the address bar on full page loads
and from the live control signals on shard re-renders, so stories read
one source either way. Fixtures and controls compose: fixtures carry the
heavy data (a `deal`), controls the light knobs (a `variant`). Do not
declare a control for a fixture-filled prop unless the body reads the
control to override it; otherwise the control writes a query key nothing
reads. The doc comment becomes the story description, and the registry
records the file holding the story for the source panel.

The annotated function passes through untouched, so `#[story]` composes
with `#[component]` and any other attribute. The story function must be a
component: the registry's render entry calls it through the component
protocol.
