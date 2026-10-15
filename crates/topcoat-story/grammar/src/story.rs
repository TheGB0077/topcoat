use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{
    Expr, ItemFn, LitStr, Meta, ReturnType, Token,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};
use topcoat_core_grammar::paths::{
    topcoat_context, topcoat_inventory, topcoat_story, topcoat_view,
};

mod kw {
    syn::custom_keyword!(group);
    syn::custom_keyword!(component);
    syn::custom_keyword!(name);
    syn::custom_keyword!(frame);
    syn::custom_keyword!(params);
}

/// One control in the story's controls panel: a query key with its widget.
pub struct StoryParam {
    kind: Ident,
    name: LitStr,
    options: Vec<LitStr>,
}

impl StoryParam {
    /// The query key the control reads and writes.
    fn key(&self) -> &LitStr {
        &self.name
    }
}

impl Parse for StoryParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let kind: Ident = input.parse()?;
        if kind != "select" && kind != "toggle" && kind != "text" {
            return Err(syn::Error::new_spanned(
                &kind,
                "expected select, toggle, or text",
            ));
        }
        let content;
        syn::parenthesized!(content in input);
        let name: LitStr = content.parse()?;
        let mut options = Vec::new();
        if !content.is_empty() {
            content.parse::<Token![,]>()?;
            let options_content;
            syn::bracketed!(options_content in content);
            while !options_content.is_empty() {
                options.push(options_content.parse::<LitStr>()?);
                if options_content.is_empty() {
                    break;
                }
                options_content.parse::<Token![,]>()?;
            }
        }
        if kind == "select" && options.is_empty() {
            return Err(syn::Error::new_spanned(
                &kind,
                "select needs at least one option",
            ));
        }
        if kind != "select" && !options.is_empty() {
            return Err(syn::Error::new_spanned(&kind, "only select takes options"));
        }
        Ok(Self {
            kind,
            name,
            options,
        })
    }
}

impl ToTokens for StoryParam {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = self.key();
        let kind = if self.kind == "select" {
            let options = &self.options;
            quote! { #topcoat_story::ParamKind::Select(&[#(#options),*]) }
        } else if self.kind == "toggle" {
            quote! { #topcoat_story::ParamKind::Toggle }
        } else {
            quote! { #topcoat_story::ParamKind::Text }
        };
        quote! { #topcoat_story::ParamDef { name: #name, kind: #kind } }.to_tokens(tokens);
    }
}

/// One fixture: a component prop value the gallery renders the story
/// with, evaluated fresh on every render.
pub struct StoryFixture {
    name: Ident,
    value: Expr,
}

impl Parse for StoryFixture {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let value: Expr = input.parse()?;
        Ok(Self { name, value })
    }
}

impl ToTokens for StoryFixture {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let value = &self.value;
        quote! { .#name(#value) }.to_tokens(tokens);
    }
}

/// The `#[story]` attribute: which sidebar entry the annotated function is.
pub struct StoryAttr {
    group: LitStr,
    component: LitStr,
    name: LitStr,
    frame: Option<Ident>,
    params: Vec<StoryParam>,
    fixtures: Vec<StoryFixture>,
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
        let mut params = Vec::new();
        let mut fixtures = Vec::new();

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
            } else if lookahead.peek(kw::params) {
                input.parse::<kw::params>()?;
                input.parse::<Token![=]>()?;
                let content;
                syn::bracketed!(content in input);
                while !content.is_empty() {
                    params.push(content.parse::<StoryParam>()?);
                    if content.is_empty() {
                        break;
                    }
                    content.parse::<Token![,]>()?;
                }
                for param in &params {
                    let duplicates = params
                        .iter()
                        .filter(|other| other.key().value() == param.key().value())
                        .count();
                    if duplicates > 1 {
                        return Err(syn::Error::new(
                            param.key().span(),
                            "duplicate control for this query key",
                        ));
                    }
                }
            } else {
                // Anything else must be a fixture: `name = expr`.
                let fork = input.fork();
                let is_fixture = fork.parse::<Ident>().is_ok() && fork.parse::<Token![=]>().is_ok();
                if !is_fixture {
                    return Err(lookahead.error());
                }
                let fixture: StoryFixture = input.parse()?;
                if fixtures
                    .iter()
                    .any(|other: &StoryFixture| other.name == fixture.name)
                {
                    return Err(syn::Error::new(
                        fixture.name.span(),
                        format!("duplicate fixture for prop `{}`", fixture.name),
                    ));
                }
                fixtures.push(fixture);
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
            params,
            fixtures,
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
    /// [`StoryAttr`] or [`StoryItem`], if the item is not a valid story
    /// function, or if the fixtures do not match its props.
    pub fn parse(attr: TokenStream, item: TokenStream) -> syn::Result<Self> {
        let attr: StoryAttr = syn::parse2(attr)?;
        let item: StoryItem = syn::parse2(item)?;
        validate_fixtures(&attr, &item)?;
        Ok(Self::new(attr, item))
    }
}

/// Checks the attribute's fixtures against the story function's props:
/// every required prop needs a fixture, every fixture needs a prop, and
/// no prop may hide behind a reserved story key.
fn validate_fixtures(attr: &StoryAttr, item: &StoryItem) -> syn::Result<()> {
    const RESERVED: &[&str] = &["group", "component", "name", "frame", "params"];

    let mut required = Vec::new();
    for input in &item.item.sig.inputs {
        let syn::FnArg::Typed(typed) = input else {
            continue;
        };
        let syn::Pat::Ident(prop) = &*typed.pat else {
            continue;
        };
        if prop.ident == "cx" {
            continue;
        }
        if RESERVED.contains(&prop.ident.to_string().as_str()) {
            return Err(syn::Error::new_spanned(
                &prop.ident,
                format!("prop `{}` collides with a reserved story key", prop.ident),
            ));
        }
        let defaulted = typed
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("default"));
        if !defaulted {
            required.push(prop.ident.clone());
        }
    }

    for prop in &required {
        if !attr.fixtures.iter().any(|fixture| fixture.name == *prop) {
            return Err(syn::Error::new(
                item.item.sig.ident.span(),
                format!("story is missing a fixture for prop `{prop}`"),
            ));
        }
    }
    for fixture in &attr.fixtures {
        let known = item.item.sig.inputs.iter().any(|input| match input {
            syn::FnArg::Typed(typed) => match &*typed.pat {
                syn::Pat::Ident(prop) => prop.ident == fixture.name,
                _ => false,
            },
            syn::FnArg::Receiver(_) => false,
        });
        if !known {
            return Err(syn::Error::new_spanned(
                &fixture.name,
                format!("no such prop `{}` on the story function", fixture.name),
            ));
        }
    }
    Ok(())
}

impl ToTokens for Story {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attr = &self.0;
        let story = &self.1;
        let item = &story.item;
        let ident = &item.sig.ident;
        let group = &attr.group;
        let component = &attr.component;
        let name = &attr.name;
        let frame = attr.frame();
        let description = LitStr::new(&story.description(), Span::call_site());
        let params = &attr.params;
        let fixtures = &attr.fixtures;

        quote! {
            #item

            const _: () = {
                fn render(
                    cx: &#topcoat_context::Cx,
                ) -> #topcoat_view::BoxView<'_> {
                    ::std::boxed::Box::pin(#topcoat_view::HoistView::new(
                        #topcoat_view::internal::ThenView::new(async move {
                            let props = <#ident as #topcoat_view::Component>::props_builder()
                                #(#fixtures)*
                                .build();
                            <#ident as #topcoat_view::Component>::render(#ident, cx, props).await
                        }),
                    ))
                }

                #topcoat_inventory::submit! {
                    #topcoat_story::StoryEntry {
                        meta: #topcoat_story::StoryMeta {
                            group: #group,
                            component: #component,
                            name: #name,
                            description: #description,
                            frame: #frame,
                            source_file: ::core::file!(),
                            params: &[#(#params),*],
                        },
                        render,
                    }
                }
            };
        }
        .to_tokens(tokens);
    }
}
