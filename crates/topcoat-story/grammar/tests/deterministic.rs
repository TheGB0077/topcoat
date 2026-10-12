use quote::{ToTokens, quote};
use topcoat_core_grammar::testing::assert_deterministic;
use topcoat_story_grammar::story::Story;

#[test]
fn story() {
    let attrs = [
        quote! { group = "Catalog", component = "Card", name = "Default" },
        quote! { name = "Dark", component = "Card", group = "Catalog", frame = Dark },
        quote! { group = "Catalog", component = "Card", name = "Bare", frame = Bare, },
    ];
    for attr in attrs {
        assert_deterministic(|| {
            Story::parse(
                attr.clone(),
                quote! {
                    /// On sale.
                    async fn sale() -> Result<impl View> {
                        Ok(view! { <p>"sale"</p> })
                    }
                },
            )
        });
    }
}

#[test]
fn expansion_registers_meta_in_submit() {
    let expanded = Story::parse(
        quote! { group = "Catalog", component = "Card", name = "Sale", frame = Dark },
        quote! {
            /// Struck-through base price.
            /// Badge emphasis.
            async fn sale() -> Result<impl View> {
                Ok(view! { <p>"sale"</p> })
            }
        },
    )
    .expect("story parses")
    .into_token_stream()
    .to_string();
    let compact: String = expanded.split_whitespace().collect();
    for part in [
        "group:\"Catalog\"",
        "component:\"Card\"",
        "name:\"Sale\"",
        "description:\"Struck-throughbaseprice.\\nBadgeemphasis.\"",
        "::topcoat_story::Frame::Dark",
        "::core::file!()",
    ] {
        let compact_part: String = part.split_whitespace().collect();
        assert!(compact.contains(&compact_part), "{expanded}");
    }
}
