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
    fixtures = [deal = fixtures::deal_card_sale(), tone = "plain".to_owned()],
)]
#[component]
async fn deal_card_discounted(deal: fixtures::Deal, tone: String, glow: bool) -> Result<impl View> {
    Ok(view! { <p>(deal.title) " " (tone)</p> })
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

`fixtures = [...]` gives every prop its default: the value the gallery
renders that prop with, evaluated fresh on every render. Every prop
without `#[default]` needs one, except `cx`; anything else is rejected,
so a misspelled prop or a missing fixture fails the build instead of the
workshop. `#[default]` props may still take a fixture overriding the
default. Fixtures live in their own list because component props can
share names with story keys — a `name` prop takes its fixture from
`fixtures = [name = ...]`, unambiguous next to the story's own `name`.

A control whose key matches a prop wires straight into it: the knob
value overrides the fixture, an absent or invalid knob falls back to it.
`toggles` pair only with `bool` props, take no fixture, and default to
off. Text, select, and toggle cover `String`, `&str`, `bool`, and the
number types. A control must name a prop it can drive; anything else is
rejected, so a misspelled control fails the build instead of sitting
dead in the workshop. Types outside the wired set keep their fixture:
show each value in its own story, the way matrix stories already do.

A `global("key", [...])` control behaves like `select` within its own
story and additionally rides along every dashboard navigation, with one
toolbar switcher per key unioned across stories. Use it for cross-cutting
context such as locales: declare `global("locale", ["en", "pt"])` in each
participating story, then resolve it application-side from the `locale`
query key (a plain `query_params` read in the locale layer). Stories
without the key ignore it; the toolbar only appears when at least one
story declares a global.

The doc comment becomes the story description, and the registry records
the file holding the story for the source panel.

The annotated function passes through untouched, so `#[story]` composes
with `#[component]` and any other attribute. The story function must be a
component: the registry's render entry calls it through the component
protocol.
