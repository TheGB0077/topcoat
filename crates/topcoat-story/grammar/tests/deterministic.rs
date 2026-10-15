use quote::quote;
use topcoat_core_grammar::testing::assert_deterministic;
use topcoat_story_grammar::story::Story;

#[test]
fn story() {
    let plain = quote! {
        /// On sale.
        async fn sale() -> Result<impl View> {
            Ok(view! { <p>"sale"</p> })
        }
    };
    let toned = quote! {
        async fn sale(tone: String) -> Result<impl View> {
            Ok(view! { <p>"sale"</p> })
        }
    };
    let flagged = quote! {
        async fn sale(glow: bool, label: String) -> Result<impl View> {
            Ok(view! { <p>"sale"</p> })
        }
    };
    let world = quote! {
        async fn sale(locale: String) -> Result<impl View> {
            Ok(view! { <p>"sale"</p> })
        }
    };
    let cases = [
        (
            quote! { group = "Catalog", component = "Card", name = "Default" },
            plain.clone(),
        ),
        (
            quote! { name = "Dark", component = "Card", group = "Catalog", frame = Dark },
            plain.clone(),
        ),
        (
            quote! { group = "Catalog", component = "Card", name = "Bare", frame = Bare, },
            plain.clone(),
        ),
        (
            quote! { group = "Catalog", component = "Card", name = "Toned", params = [select("tone", ["plain", "loud"])], fixtures = [tone = "plain".to_owned()] },
            toned,
        ),
        (
            quote! { group = "Catalog", component = "Card", name = "Flagged", params = [toggle("glow"), text("label")], fixtures = [label = String::new()] },
            flagged,
        ),
        (
            quote! { group = "Catalog", component = "Card", name = "World", params = [global("locale", ["en", "pt"])], fixtures = [locale = "en".to_owned()] },
            world,
        ),
    ];
    for (attr, item) in cases {
        assert_deterministic(|| Story::parse(attr.clone(), item.clone()));
    }
}

#[test]
fn story_with_fixtures() {
    let attrs = [
        quote! { group = "Catalog", component = "Card", name = "Sale", fixtures = [deal = fixtures::sale()] },
        quote! { group = "Catalog", component = "Card", name = "Full", fixtures = [deal = fixtures::sale(), count = 3] },
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
            quote! { group = "Catalog", component = "Card", name = "Typo", fixtures = [dael = fixtures::sale()] },
            item.clone(),
        )
        .is_err(),
        "a fixture without a prop fails"
    );
    assert!(
        Story::parse(
            quote! { group = "Catalog", component = "Card", name = "Ok", fixtures = [deal = fixtures::sale()] },
            item.clone(),
        )
        .is_ok(),
        "a matching fixture passes"
    );
    assert!(
        Story::parse(
            quote! { group = "Catalog", component = "Card", name = "Empty", params = [global("locale", [])] },
            item.clone(),
        )
        .is_err(),
        "a global without options fails"
    );
}
