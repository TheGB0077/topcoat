use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{
    Expr, ItemFn, LitStr, Meta, ReturnType, Token,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};
use topcoat_core_grammar::paths::{topcoat_inventory, topcoat_story};

mod kw {
    syn::custom_keyword!(group);
    syn::custom_keyword!(component);
    syn::custom_keyword!(name);
    syn::custom_keyword!(frame);
}

/// The `#[story]` attribute: which sidebar entry the annotated function is.
pub struct StoryAttr {
    group: LitStr,
    component: LitStr,
    name: LitStr,
    frame: Option<Ident>,
}

impl StoryAttr {
    /// The gallery frame path for the declared frame, defaulting to padded.
    fn frame(&self) -> TokenStream {
        let frame = self
            .frame
            .as_ref()
            .map_or_else(|| Ident::new("Padded", Span::call_site()), Clone::clone);
        quote! { #topcoat_story::Frame::#frame }
    }
}

impl Parse for StoryAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut group = None;
        let mut component = None;
        let mut name = None;
        let mut frame = None;

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::group) {
                input.parse::<kw::group>()?;
                input.parse::<Token![=]>()?;
                group = Some(input.parse::<LitStr>()?);
            } else if lookahead.peek(kw::component) {
                input.parse::<kw::component>()?;
                input.parse::<Token![=]>()?;
                component = Some(input.parse::<LitStr>()?);
            } else if lookahead.peek(kw::name) {
                input.parse::<kw::name>()?;
                input.parse::<Token![=]>()?;
                name = Some(input.parse::<LitStr>()?);
            } else if lookahead.peek(kw::frame) {
                input.parse::<kw::frame>()?;
                input.parse::<Token![=]>()?;
                let variant: Ident = input.parse()?;
                if !["Padded", "Bare", "Dark", "FullBleed"]
                    .iter()
                    .any(|known| variant == known)
                {
                    return Err(syn::Error::new_spanned(
                        &variant,
                        "unknown frame: expected Padded, Bare, Dark, or FullBleed",
                    ));
                }
                frame = Some(variant);
            } else {
                return Err(lookahead.error());
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        let missing = if group.is_none() {
            Some("group")
        } else if component.is_none() {
            Some("component")
        } else if name.is_none() {
            Some("name")
        } else {
            None
        };
        if let Some(missing) = missing {
            return Err(syn::Error::new(
                Span::call_site(),
                format!("story is missing `{missing}`"),
            ));
        }

        Ok(Self {
            group: group.expect("group is present"),
            component: component.expect("component is present"),
            name: name.expect("name is present"),
            frame,
        })
    }
}

/// The annotated function that renders the story: an async function
/// declaring a return type, passed through untouched.
pub struct StoryItem {
    item: ItemFn,
}

impl StoryItem {
    /// The function's doc comment, shown as the story description.
    fn description(&self) -> String {
        let mut lines = Vec::new();
        for attr in &self.item.attrs {
            let Meta::NameValue(meta) = &attr.meta else {
                continue;
            };
            if !meta.path.is_ident("doc") {
                continue;
            }
            let Expr::Lit(lit) = &meta.value else {
                continue;
            };
            let syn::Lit::Str(line) = &lit.lit else {
                continue;
            };
            let value = line.value();
            lines.push(value.strip_prefix(' ').unwrap_or(&value).to_owned());
        }
        lines.join("\n")
    }
}

impl Parse for StoryItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let item: ItemFn = input.parse()?;
        if item.sig.asyncness.is_none() {
            return Err(syn::Error::new(
                item.sig.fn_token.span(),
                "story functions must be async",
            ));
        }
        if let ReturnType::Default = &item.sig.output {
            return Err(syn::Error::new(
                item.sig.fn_token.span(),
                "story functions must declare a return type",
            ));
        }
        Ok(Self { item })
    }
}

pub struct Story(StoryAttr, StoryItem);

impl Story {
    #[must_use]
    pub fn new(attr: StoryAttr, item: StoryItem) -> Self {
        Self(attr, item)
    }

    /// Parses a story attribute and item from token streams.
    ///
    /// # Errors
    ///
    /// Returns an error if either token stream fails to parse as a
    /// [`StoryAttr`] or [`StoryItem`], or if the item is not a valid story
    /// function.
    pub fn parse(attr: TokenStream, item: TokenStream) -> syn::Result<Self> {
        Ok(Self::new(syn::parse2(attr)?, syn::parse2(item)?))
    }
}

impl ToTokens for Story {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attr = &self.0;
        let story = &self.1;
        let item = &story.item;
        let group = &attr.group;
        let component = &attr.component;
        let name = &attr.name;
        let frame = attr.frame();
        let description = LitStr::new(&story.description(), Span::call_site());

        quote! {
            #item

            const _: () = {
                #topcoat_inventory::submit! {
                    #topcoat_story::StoryEntry {
                        meta: #topcoat_story::StoryMeta {
                            group: #group,
                            component: #component,
                            name: #name,
                            description: #description,
                            frame: #frame,
                            source_file: ::core::file!(),
                        }
                    }
                }
            };
        }
        .to_tokens(tokens);
    }
}
