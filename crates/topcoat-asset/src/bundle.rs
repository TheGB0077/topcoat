use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{AssetCatalog, AssetId, BundledAsset, MANIFEST_NAME, Manifest, RawAsset};

/// Bundle location for a deployed binary: the `assets` directory beside the
/// executable.
const DEPLOY_SUBDIR: &str = "assets";

/// A directory of bundled files and the catalog used to look them up.
///
/// Built by the [`Bundler`](crate::Bundler) and loaded at runtime via
/// [`AssetBundle::load`] or [`AssetBundle::load_dir`].
#[derive(Debug, Clone)]
pub struct AssetBundle {
    pub(crate) dir: PathBuf,
    pub(crate) catalog: AssetCatalog,
}

impl AssetBundle {
    /// Loads the asset bundle for the running binary.
    ///
    /// [`asset!`](crate::asset) records the declaring crate's manifest
    /// directory inside the binary, and `load` resolves that crate's
    /// `<crate>/dist/assets` bundle from the executable's records. When no
    /// dist bundle is found, as on a deploy host, the bundle is loaded from
    /// the deploy location next to the executable; place it there from the
    /// same build.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::NotFound`] if none of the candidate
    /// directories contain a bundle manifest. Other errors can occur when
    /// locating the executable or loading the manifest through
    /// [`load_dir`](Self::load_dir).
    pub fn load() -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let dist = Self::dist_dir(&exe);
        let deploy_dir = exe
            .parent()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "current executable has no parent directory",
                )
            })?
            .join(DEPLOY_SUBDIR);

        match (dist, deploy_dir.join(MANIFEST_NAME).is_file()) {
            (Some(dist), _) if dist.join(MANIFEST_NAME).is_file() => Self::load_dir(dist),
            (_, true) => Self::load_dir(deploy_dir),
            (dist, false) => Err(io::Error::new(
                io::ErrorKind::NotFound,
                match dist {
                    Some(dist) => format!(
                        "no asset bundle at {}, {} (run `topcoat asset bundle`)",
                        dist.display(),
                        deploy_dir.display()
                    ),
                    None => format!(
                        "no asset bundle at {} (run `topcoat asset bundle`)",
                        deploy_dir.display()
                    ),
                },
            )),
        }
    }

    /// `dist/assets` of the package that owns the executable, found among
    /// the embedded `asset!` records by the package name matching the
    /// executable's file stem.
    fn dist_dir(exe: &Path) -> Option<PathBuf> {
        let stem = exe.file_stem()?.to_str()?;
        let bytes = fs::read(exe).ok()?;
        Self::dist_dir_from(&RawAsset::find_in_binary(&bytes), stem)
    }

    fn dist_dir_from(records: &[RawAsset], stem: &str) -> Option<PathBuf> {
        records
            .iter()
            .find(|record| record.package_name() == stem)
            .map(|record| Path::new(record.manifest_dir()).join("dist").join("assets"))
    }

    /// Load a bundle from a specific directory.
    ///
    /// `dir` must contain `manifest.toml` and the bundled files. Relative paths
    /// start from the process working directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read or parsed, or if it
    /// reports an unsupported version.
    pub fn load_dir(dir: impl AsRef<Path>) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let manifest = Manifest::load(dir.join(MANIFEST_NAME))?;
        Ok(Self {
            dir,
            catalog: manifest.into(),
        })
    }

    /// Directory the bundle was loaded from.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The catalog mapping [`AssetId`]s to the files in the bundle directory.
    #[must_use]
    pub fn catalog(&self) -> &AssetCatalog {
        &self.catalog
    }

    /// Look up the bundled file for an [`AssetId`].
    ///
    /// Join the returned filename to [`dir`](Self::dir) to locate it on disk.
    #[must_use]
    pub fn get(&self, id: AssetId) -> Option<&BundledAsset> {
        self.catalog.get(id)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::AssetOptions;

    /// Encoded records for one resolution scenario: the package `package_name`
    /// declares the asset at `manifest_dir`.
    fn records(scenarios: &[(&str, &str)]) -> Vec<RawAsset> {
        scenarios
            .iter()
            .map(|&(package_name, manifest_dir)| {
                const OPTIONS: AssetOptions = AssetOptions::NONE;
                let encoded = RawAsset::encode(
                    AssetId::new("app", "src/main.rs", "assets/logo.png", &OPTIONS),
                    "assets/logo.png",
                    package_name,
                    manifest_dir,
                    "src/main.rs",
                    &OPTIONS,
                );
                RawAsset::decode(&encoded).unwrap()
            })
            .collect()
    }

    fn dist(dir: &str) -> PathBuf {
        Path::new(dir).join("dist").join("assets")
    }

    #[test]
    fn dist_dir_comes_from_the_binary_owning_package() {
        let records = records(&[
            ("other-app", "/workspace/other-app"),
            ("my-app", "/workspace/my-app"),
        ]);

        let dir = AssetBundle::dist_dir_from(&records, "my-app");

        assert_eq!(dir, Some(dist("/workspace/my-app")));
    }

    #[test]
    fn dist_dir_is_none_without_the_owning_package() {
        let records = records(&[("other", "/workspace/other")]);

        let dir = AssetBundle::dist_dir_from(&records, "my-app");

        assert_eq!(dir, None);
    }
}
