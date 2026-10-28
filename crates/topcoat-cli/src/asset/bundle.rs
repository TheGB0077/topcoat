use std::{error::Error, path::PathBuf};

use clap::Args;
use console::style;
use topcoat_asset::ASSETS_SUBDIR;

use super::{CACHE_SCOPE, DIST_SUBDIR};
use crate::common::cargo::{BuildFlags, BuildOpts, BuiltTarget, Metadata};

#[derive(Args)]
pub(super) struct BundleArgs {
    #[command(flatten)]
    build: BuildFlags,
    /// Output directory for the bundle (defaults to `<crate>/dist/assets`)
    #[arg(short, long)]
    out: Option<PathBuf>,
}

pub(super) async fn run(args: BundleArgs) {
    let (target, bytes) = BuildOpts::from(args.build)
        .build_and_read(|_, _| {})
        .await
        .unwrap_or_else(|e| e.print_and_exit());

    let out_dir = match run_bundle(&target, &bytes, args.out).await {
        Ok(path) => path,
        Err(error) => {
            eprintln!(
                "{}",
                style(format!("failed to bundle assets: {error}")).red()
            );
            std::process::exit(1);
        }
    };

    println!("bundled assets into {}", out_dir.display());
}

/// Bundles assets declared in the executable.
///
/// `bytes` must contain the file at `target.path`. Writes the bundle to
/// `out_override`, or to the `dist/assets` directory of the package Cargo
/// reported building `target` by default. Fails when that package cannot be
/// identified; pass `--out` in that case.
///
/// `topcoat dev` writes the same location and names it to the application it
/// starts through `TOPCOAT_ASSETS_PATH`; a deploy ships the bundle as the
/// `assets` directory beside the binary.
pub(crate) async fn run_bundle(
    target: &BuiltTarget,
    bytes: &[u8],
    out_override: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let out_dir = match out_override {
        Some(dir) => dir,
        None => {
            let metadata = Metadata::workspace()
                .await
                .ok_or("could not query cargo metadata")?;
            let crate_dir = target
                .package
                .as_deref()
                .and_then(|package| metadata.package_dir(package))
                .ok_or_else(|| {
                    format!(
                        "could not identify the crate that owns {}; pass --out",
                        target.path.display()
                    )
                })?;
            crate_dir.join(DIST_SUBDIR).join(ASSETS_SUBDIR)
        }
    };
    let cache_dir = topcoat_core::cache::user_cache_dir(CACHE_SCOPE)
        .ok_or("no user cache directory for asset downloads (set XDG_CACHE_HOME or HOME)")?;

    // `bundle` blocks on filesystem and network I/O, so run it off the runtime.
    let bytes = bytes.to_vec();
    let bundle_dir = out_dir.clone();
    tokio::task::spawn_blocking(move || {
        let config = topcoat_asset::BundlerConfig::new().cache_dir(cache_dir);
        topcoat_asset::Bundler::new(&config).bundle(&bytes, &bundle_dir)
    })
    .await??;
    Ok(out_dir)
}
