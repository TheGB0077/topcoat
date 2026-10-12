# `#[story]`

Registers a story function in the workshop registry.

```rust
use topcoat::{Result, story::story, view::{View, component, view}};

/// Struck-through base price with badge emphasis.
#[story(group = "Catalog", component = "DealCard", name = "Discounted", frame = Padded)]
#[component]
async fn deal_card_discounted() -> Result<impl View> {
    Ok(view! { <p>"discounted"</p> })
}
```

`group`, `component`, and `name` place the story in the sidebar tree, in
any order. `frame` is one of `Padded`, `Bare`, `Dark`, or `FullBleed`, and
defaults to `Padded`. The doc comment becomes the story description, and the
registry records the file holding the story for the source panel.

The annotated function passes through untouched, so `#[story]` composes
with `#[component]` and any other attribute.
