# `#[story]`

Registers a story function in the workshop registry.

```rust
use topcoat::{Result, story::story, view::{View, component, view}};

/// Struck-through base price with badge emphasis.
#[story(
    group = "Catalog",
    component = "DealCard",
    name = "Discounted",
    frame = Padded,
    params = [select("tone", ["plain", "loud"]), toggle("glow")],
)]
#[component]
async fn deal_card_discounted() -> Result<impl View> {
    Ok(view! { <p>"discounted"</p> })
}
```

`group`, `component`, and `name` place the story in the sidebar tree, in
any order. `frame` is one of `Padded`, `Bare`, `Dark`, or `FullBleed`, and
defaults to `Padded`. `params` declares the controls panel: `select` with
its options, `toggle` for on/off, and `text` for free input. Each control
reads and writes one query key, which the story body reads through
`story::controls` with an ordinary deserializable struct of optional
fields, falling back to `Default` when the query is missing or malformed:

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
one source either way. The doc comment becomes the story description,
and the registry records the file holding the story for the source
panel.

The annotated function passes through untouched, so `#[story]` composes
with `#[component]` and any other attribute. The story function must be a
component: the registry's render entry calls it through the component
protocol.
