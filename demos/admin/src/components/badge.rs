use topcoat::{
    Result,
    story::story,
    view::{Attributes, Child, Class, StaticClass, View, class, component, view},
};

/// The visual style of a [`badge`].
///
/// [`Default`] is `BadgeVariant::Primary`, used when no variant is given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BadgeVariant {
    /// The primary-filled badge for highlighted statuses.
    #[default]
    Primary,
    /// A muted, tinted fill for neutral statuses.
    Secondary,
    /// A hairline-bordered badge on the page background.
    Outline,
    /// A destructive-filled badge for errors and warnings.
    Destructive,
}

impl std::str::FromStr for BadgeVariant {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Primary" => Ok(Self::Primary),
            "Secondary" => Ok(Self::Secondary),
            "Outline" => Ok(Self::Outline),
            "Destructive" => Ok(Self::Destructive),
            _ => Err(()),
        }
    }
}

impl BadgeVariant {
    /// Classes for the variant, including its border color. Keep border colors out of
    /// the shared base to avoid conflicting classes.
    fn classes(self) -> StaticClass {
        match self {
            Self::Primary => class!("border-transparent bg-primary text-primary-foreground"),
            Self::Secondary => class!("border-transparent bg-foreground/5 text-foreground"),
            Self::Outline => class!("border-border text-foreground"),
            Self::Destructive => {
                class!("border-transparent bg-destructive text-destructive-foreground")
            }
        }
    }
}

/// Classes shared by badge variants. A border reserves the same space in every variant.
const BASE: StaticClass = class!(
    "inline-flex w-fit shrink-0 items-center justify-center gap-1 rounded-md",
    "border px-2 py-0.5 text-xs font-medium whitespace-nowrap",
);

/// Builds the full class list for a badge of the given `variant`.
///
/// Use it to give badge styling to another element, such as a link:
///
/// ```ignore
/// view! {
///     <a href="/releases/v2" class=(badge_variants(BadgeVariant::Outline))>"v2.0"</a>
/// }
/// ```
#[must_use]
pub fn badge_variants(variant: BadgeVariant) -> Class<(StaticClass, StaticClass)> {
    class!(BASE, variant.classes())
}

/// A small label for a status or count.
///
/// `variant` defaults to `Primary`. Pass the label as children and extra attributes
/// through `attrs`. Attributes go on the `<span>`, with classes added to its classes.
/// Use [`badge_variants`] to apply the same styling to another element.
///
/// ```ignore
/// view! {
///     badge(variant: BadgeVariant::Destructive, "Failed")
/// }
/// ```
#[story(group = "Admin/Decor", component = "Badge", name = "Default", params = [select("variant", ["Primary", "Secondary", "Outline", "Destructive"])], fixtures = [variant = BadgeVariant::Primary])]
#[component]
pub async fn badge(
    #[default] variant: BadgeVariant,
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <span class=(class!(BASE, variant.classes(), attrs.remove("class"))) (attrs)>
            (child)
        </span>
    })
}
