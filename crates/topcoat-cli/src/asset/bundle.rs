use std::{
    error::Error,
    path::{Path, PathBuf},
};

use clap::Args;
use console::style;

use super::{CACHE_SCOPE, DIST_SUBDIR, OUT_SUBDIR};
use crate::common::cargo::{BuildFlags, BuildOpts, Metadata};

#[derive(Args)]
pub(super) struct BundleArgs {
    #[command(flatten)]
    build: BuildFlags,
    /// Output directory for the bundle (defaults to `<crate>/dist/assets`)
    #[arg(short, long)]
    out: Option<PathBuf>,
}

pub(super) async fn run(args: BundleArgs) {
    let (exe, bytes) = BuildOpts::from(args.build)
        .build_and_read(|_, _| {})
        .await
        .unwrap_or_else(|e| e.print_and_exit());

    let out_dir = match run_bundle(&exe, &bytes, args.out).await {
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
/// `bytes` must contain the file at `exe`. Writes the bundle to `out_override`,
/// or to `<crate>/dist/assets` by default. Fails when the owning crate cannot
/// be identified; pass `--out` in that case.
pub(crate) async fn run_bundle(
    exe: &Path,
    bytes: &[u8],
    out_override: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let out_dir = match out_override {
        Some(dir) => dir,
        None => {
            let metadata = Metadata::workspace()
                .await
                .ok_or("could not query cargo metadata")?;
            let stem = exe
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or("built executable has no file stem")?;
            metadata
                .bin_package_dir(stem)
                .map(|crate_dir| crate_dir.join(DIST_SUBDIR).join(OUT_SUBDIR))
                .ok_or_else(|| {
                    format!(
                        "could not identify the crate that owns {}; pass --out",
                        exe.display()
                    )
                })?
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
