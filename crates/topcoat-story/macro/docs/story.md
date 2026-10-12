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
reads and writes one query key, which the story body reads through an
ordinary `#[query_params]` struct, falling back to defaults when a value
is missing or invalid. The doc comment becomes the story description, and
the registry records the file holding the story plus its source text for
the source panel.

The annotated function passes through untouched, so `#[story]` composes
with `#[component]` and any other attribute. The story function must be a
component: the registry's render entry calls it through the component
protocol.
