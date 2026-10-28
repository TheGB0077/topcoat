use std::path::PathBuf;

use clap::Args;
use console::style;

use super::{CACHE_SCOPE, DIST_SUBDIR};

#[derive(Args)]
pub(super) struct CleanArgs {
    /// Asset bundle directory to remove (defaults to every crate's
    /// `dist/assets` in the workspace, plus the asset download cache)
    #[arg(short, long)]
    out: Option<PathBuf>,
}

pub(super) async fn run(args: CleanArgs) {
    let dirs = match args.out {
        Some(dir) => vec![dir],
        None => {
            let Some(metadata) = crate::common::cargo::Metadata::workspace().await else {
                eprintln!(
                    "{}",
                    style("could not query cargo metadata; pass --out").red()
                );
                std::process::exit(1);
            };
            let mut dirs: Vec<PathBuf> = metadata
                .local_package_dirs()
                .into_iter()
                .map(|dir| dir.join(DIST_SUBDIR).join(topcoat_asset::ASSETS_SUBDIR))
                .filter(|dir| dir.join(topcoat_asset::MANIFEST_NAME).is_file())
                .collect();
            dirs.extend(topcoat_core::cache::user_cache_dir(CACHE_SCOPE));
            dirs
        }
    };

    for dir in &dirs {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => println!("removed {}", dir.display()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                eprintln!(
                    "{}",
                    style(format!("failed to remove {}: {error}", dir.display())).red()
                );
                std::process::exit(1);
            }
        }
    }
}
