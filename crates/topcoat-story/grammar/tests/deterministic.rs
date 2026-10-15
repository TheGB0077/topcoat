use quote::{ToTokens, quote};
use topcoat_core_grammar::testing::assert_deterministic;
use topcoat_story_grammar::story::Story;

#[test]
fn story() {
    let attrs = [
        quote! { group = "Catalog", component = "Card", name = "Default" },
        quote! { name = "Dark", component = "Card", group = "Catalog", frame = Dark },
        quote! { group = "Catalog", component = "Card", name = "Bare", frame = Bare, },
        quote! { group = "Catalog", component = "Card", name = "Toned", params = [select("tone", ["plain", "loud"])] },
        quote! { group = "Catalog", component = "Card", name = "Flagged", params = [toggle("glow"), text("label"),] },
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
fn story_with_fixtures() {
    let attrs = [
        quote! { group = "Catalog", component = "Card", name = "Sale", deal = fixtures::sale() },
        quote! { group = "Catalog", component = "Card", name = "Full", deal = fixtures::sale(), count = 3, },
    ];
    for attr in attrs {
        assert_deterministic(|| {
            Story::parse(
                attr.clone(),
                quote! {
                    async fn sale(deal: Deal, #[default] count: usize) -> Result<impl View> {
                        Ok(view! { <p>"sale"</p> })
                    }
                },
            )
        });
    }
}

#[test]
fn fixtures_must_match_props() {
    let item = quote! {
        async fn sale(deal: Deal) -> Result<impl View> {
            Ok(view! { <p>"sale"</p> })
        }
    };
    assert!(
        Story::parse(
            quote! { group = "Catalog", component = "Card", name = "Missing" },
            item.clone(),
        )
        .is_err(),
        "a required prop without a fixture fails"
    );
    assert!(
        Story::parse(
            quote! { group = "Catalog", component = "Card", name = "Typo", dael = fixtures::sale() },
            item.clone(),
        )
        .is_err(),
        "a fixture without a prop fails"
    );
    assert!(
        Story::parse(
            quote! { group = "Catalog", component = "Card", name = "Ok", deal = fixtures::sale() },
            item.clone(),
        )
        .is_ok(),
        "a matching fixture passes"
    );
}

#[test]
fn expansion_registers_meta_in_submit() {
    let expanded = Story::parse(
        quote! { group = "Catalog", component = "Card", name = "Sale", frame = Dark, params = [select("tone", ["plain", "loud"]), toggle("glow")] },
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
        "ParamDef{name:\"tone\",kind:::topcoat_story::ParamKind::Select(&[\"plain\",\"loud\"])}",
        "ParamDef{name:\"glow\",kind:::topcoat_story::ParamKind::Toggle}",
        "fnrender(",
    ] {
        let compact_part: String = part.split_whitespace().collect();
        assert!(compact.contains(&compact_part), "{expanded}");
    }
}
