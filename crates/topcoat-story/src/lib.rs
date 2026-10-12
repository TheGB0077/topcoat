//! The story registry behind the designbook workshop.
//!
//! Every `#[story]` in the binary submits a [`StoryEntry`] here through
//! link-time collection, following the same mechanism as routes, pages, and
//! fonts. Collection is unconditional: enabling this crate opts the binary
//! into gathering stories. [`stories`] returns the entries sorted by group,
//! component, then story name, so sidebar order and story URLs stay stable
//! regardless of link order.

mod registry;

pub use registry::*;
