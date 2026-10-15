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
    syn::custom_keyword!(fixtures);
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
        if kind != "select" && kind != "toggle" && kind != "text" && kind != "global" {
            return Err(syn::Error::new_spanned(
                &kind,
                "expected select, toggle, text, or global",
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
        if (kind == "select" || kind == "global") && options.is_empty() {
            return Err(syn::Error::new_spanned(
                &kind,
                format!("{kind} needs at least one option"),
            ));
        }
        if kind != "select" && kind != "global" && !options.is_empty() {
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
        } else if self.kind == "global" {
            let options = &self.options;
            quote! { #topcoat_story::ParamKind::Global(&[#(#options),*]) }
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
            } else if lookahead.peek(kw::fixtures) {
                input.parse::<kw::fixtures>()?;
                input.parse::<Token![=]>()?;
                let content;
                syn::bracketed!(content in input);
                while !content.is_empty() {
                    let fixture: StoryFixture = content.parse()?;
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
                    if content.is_empty() {
                        break;
                    }
                    content.parse::<Token![,]>()?;
                }
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

pub struct Story(StoryAttr, StoryItem, Vec<PropPlan>);

impl Story {
    #[must_use]
    pub(crate) fn new(attr: StoryAttr, item: StoryItem, plans: Vec<PropPlan>) -> Self {
        Self(attr, item, plans)
    }

    /// Parses a story attribute and item from token streams.
    ///
    /// # Errors
    ///
    /// Returns an error if either token stream fails to parse as a
    /// [`StoryAttr`] or [`StoryItem`], if the item is not a valid story
    /// function, or if the fixtures and controls do not match its props.
    pub fn parse(attr: TokenStream, item: TokenStream) -> syn::Result<Self> {
        let attr: StoryAttr = syn::parse2(attr)?;
        let item: StoryItem = syn::parse2(item)?;
        let plans = plan_props(&attr, &item)?;
        Ok(Self::new(attr, item, plans))
    }
}

/// How a control value converts into a component prop.
enum WireType {
    Str,
    String,
    Bool,
    Int(Ident),
    Float(Ident),
    Opt(Box<WireType>),
    Skip,
}

/// Maps a prop type to its control conversion, if one exists.
fn wire_type(ty: &syn::Type) -> WireType {
    match ty {
        syn::Type::Path(path) if path.qself.is_none() => {
            let Some(segment) = path.path.segments.last() else {
                return WireType::Skip;
            };
            if segment.ident == "Option" {
                let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
                    return WireType::Skip;
                };
                let Some(syn::GenericArgument::Type(inner)) = args.args.first() else {
                    return WireType::Skip;
                };
                return match wire_type(inner) {
                    WireType::Skip => WireType::Skip,
                    inner => WireType::Opt(Box::new(inner)),
                };
            }
            if !matches!(segment.arguments, syn::PathArguments::None) {
                return WireType::Skip;
            }
            match segment.ident.to_string().as_str() {
                "String" => WireType::String,
                "bool" => WireType::Bool,
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
                | "u128" | "usize" => WireType::Int(segment.ident.clone()),
                "f32" | "f64" => WireType::Float(segment.ident.clone()),
                _ => WireType::Skip,
            }
        }
        syn::Type::Reference(reference) if reference.mutability.is_none() => {
            match &*reference.elem {
                syn::Type::Path(path)
                    if path.qself.is_none()
                        && path.path.segments.last().is_some_and(|segment| {
                            segment.ident == "str"
                                && matches!(segment.arguments, syn::PathArguments::None)
                        }) =>
                {
                    WireType::Str
                }
                _ => WireType::Skip,
            }
        }
        _ => WireType::Skip,
    }
}

/// One prop's wiring: its fixture, and its control when a matching knob
/// feeds it.
pub(crate) struct PropPlan {
    name: Ident,
    wire: WireType,
    fixture: Option<Expr>,
    control: Option<Ident>,
}

/// Plans every prop from the signature, the fixtures, and the controls.
/// Controls must name a prop they can drive; every other prop needs a
/// fixture unless the builder defaults it.
fn plan_props(attr: &StoryAttr, item: &StoryItem) -> syn::Result<Vec<PropPlan>> {
    for param in &attr.params {
        let controlled = item.item.sig.inputs.iter().any(|input| match input {
            syn::FnArg::Typed(typed) => match &*typed.pat {
                syn::Pat::Ident(prop) => prop.ident == param.key().value(),
                _ => false,
            },
            syn::FnArg::Receiver(_) => false,
        });
        if !controlled {
            return Err(syn::Error::new_spanned(
                param.key(),
                format!(
                    "control `{}` matches no prop; knobs attach to props",
                    param.key().value()
                ),
            ));
        }
    }

    let mut plans = Vec::new();
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
        if prop.ident == "child"
            && !typed
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("default"))
        {
            return Err(syn::Error::new_spanned(
                &prop.ident,
                "pass children from a wrapper story function; attributes cannot carry markup",
            ));
        }
        let control = attr
            .params
            .iter()
            .find_map(|param| (prop.ident == param.key().value()).then(|| param.kind.clone()));
        let fixture = attr
            .fixtures
            .iter()
            .find(|fixture| fixture.name == prop.ident)
            .map(|fixture| fixture.value.clone());
        let defaulted = typed
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("default"));
        let wire = wire_type(&typed.ty);

        if let Some(kind) = &control {
            if kind == "toggle" {
                if !matches!(wire, WireType::Bool) {
                    return Err(syn::Error::new_spanned(kind, "toggle needs a bool prop"));
                }
                if fixture.is_some() {
                    return Err(syn::Error::new_spanned(
                        kind,
                        "toggle needs no fixture; it defaults to off",
                    ));
                }
            } else if matches!(&wire, WireType::Skip)
                || matches!(&wire, WireType::Opt(inner) if matches!(**inner, WireType::Skip))
            {
                return Err(syn::Error::new_spanned(
                    kind,
                    format!(
                        "cannot wire control `{}` to a custom-typed prop; show each value in its own story",
                        prop.ident
                    ),
                ));
            } else if fixture.is_none() {
                return Err(syn::Error::new(
                    item.item.sig.ident.span(),
                    format!("control `{}` needs a fixture for its default", prop.ident),
                ));
            }
        } else if !defaulted && fixture.is_none() {
            return Err(syn::Error::new(
                item.item.sig.ident.span(),
                format!("story is missing a fixture for prop `{}`", prop.ident),
            ));
        }

        plans.push(PropPlan {
            name: prop.ident.clone(),
            wire,
            fixture,
            control,
        });
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
    Ok(plans)
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

        let mut setters = Vec::new();
        for plan in &self.2 {
            let prop = &plan.name;
            let key = prop.to_string();
            let fixture = plan.fixture.as_ref().map(|value| {
                quote! { #value }
            });
            let control = plan.control.as_ref().map(ToString::to_string);
            match (&plan.wire, control.as_deref(), fixture) {
                (_, Some("toggle"), _) => {
                    setters.push(quote! {
                        .#prop(#topcoat_story::pair(pairs, #key).is_some_and(|value| value == "true"))
                    });
                }
                (WireType::String, Some(_), Some(fixture)) => {
                    setters.push(quote! {
                        .#prop(#topcoat_story::pair(pairs, #key).map(ToOwned::to_owned).unwrap_or_else(|| #fixture))
                    });
                }
                (WireType::Str, Some(_), Some(fixture)) => {
                    setters.push(quote! {
                        .#prop(#topcoat_story::pair(pairs, #key).unwrap_or(#fixture))
                    });
                }
                (WireType::Bool, Some(_), Some(fixture)) => {
                    setters.push(quote! {
                        .#prop(
                            #topcoat_story::pair(pairs, #key)
                                .and_then(|value| match value {
                                    "true" => ::std::option::Option::Some(true),
                                    "false" => ::std::option::Option::Some(false),
                                    _ => ::std::option::Option::None,
                                })
                                .unwrap_or_else(|| #fixture),
                        )
                    });
                }
                (WireType::Int(ty) | WireType::Float(ty), Some(_), Some(fixture)) => {
                    setters.push(quote! {
                        .#prop(
                            #topcoat_story::pair(pairs, #key)
                                .and_then(|value| value.parse::<#ty>().ok())
                                .unwrap_or_else(|| #fixture),
                        )
                    });
                }
                (WireType::Opt(inner), Some(_), Some(fixture)) => {
                    let merge = match &**inner {
                        WireType::String => quote! {
                            #topcoat_story::pair(pairs, #key)
                                .map(ToOwned::to_owned)
                                .or(#fixture)
                        },
                        WireType::Str => quote! {
                            #topcoat_story::pair(pairs, #key).or(#fixture)
                        },
                        WireType::Bool => quote! {
                            #topcoat_story::pair(pairs, #key)
                                .and_then(|value| match value {
                                    "true" => ::std::option::Option::Some(true),
                                    "false" => ::std::option::Option::Some(false),
                                    _ => ::std::option::Option::None,
                                })
                                .or(#fixture)
                        },
                        WireType::Int(ty) | WireType::Float(ty) => quote! {
                            #topcoat_story::pair(pairs, #key)
                                .and_then(|value| value.parse::<#ty>().ok())
                                .or(#fixture)
                        },
                        WireType::Opt(_) | WireType::Skip => {
                            quote! { #fixture }
                        }
                    };
                    setters.push(quote! { .#prop(#merge) });
                }
                (_, _, Some(fixture)) => {
                    setters.push(quote! { .#prop(#fixture) });
                }
                (_, _, None) => {}
            }
        }

        // Registration expands only with the `collect` feature, the way
        // pages register only with `discover`: without it a story is just
        // its component, so production builds carry nothing workshop-only.
        let registration = cfg!(feature = "collect").then(|| {
            quote! {
                const _: () = {
                    fn render(
                        cx: &#topcoat_context::Cx,
                        query: ::std::string::String,
                    ) -> #topcoat_story::StoryFuture<'_> {
                        ::std::boxed::Box::pin(async move {
                            use #topcoat_view::ViewExt as _;
                            let pairs = #topcoat_story::scoped_pairs(cx, &query);
                            let props = <#ident as #topcoat_view::Component>::props_builder()
                                #(#setters)*
                                .build();
                            ::std::result::Result::Ok(
                                <#ident as #topcoat_view::Component>::render(#ident, cx, props)
                                    .await?
                                    .boxed(),
                            )
                        })
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
        });

        quote! {
            #item

            #registration
        }
        .to_tokens(tokens);
    }
}
