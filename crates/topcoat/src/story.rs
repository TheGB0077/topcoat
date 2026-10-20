#[cfg(feature = "design-story")]
use std::io;

pub use topcoat_story::*;
pub use topcoat_story_macro::*;

/// Serves the story dashboard in place of the application's own router.
///
/// A build made with the `design-story` feature routes here from
/// [`start`](crate::start): the dashboard owns the whole server, so the
/// stories render the way the application renders them while nothing of the
/// application's own routing is served.
#[cfg(feature = "design-story")]
pub(crate) async fn start_dashboard() -> Result<(), io::Error> {
    use crate::router::Router;

    let builder = Router::builder();
    #[cfg(feature = "asset")]
    let builder = {
        use crate::asset::{AssetBundle, RouterBuilderAssetExt};

        match AssetBundle::load() {
            Ok(bundle) => builder.assets(bundle),
            Err(_) => builder,
        }
    };
    #[cfg(feature = "runtime")]
    let builder = {
        use crate::runtime::RouterBuilderRuntimeExt;

        builder.runtime()
    };
    crate::serve::start_service(builder.dashboard().build()).await
}
