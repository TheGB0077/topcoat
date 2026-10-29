use std::{
    collections::HashSet,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use crate::{AssetCatalog, AssetId, BundledAsset, MANIFEST_NAME, Manifest, RawAsset};

/// The name of the directory that holds a bundle: under a crate's `dist`
/// while it is staged, and beside a deployed executable.
pub const ASSETS_SUBDIR: &str = "assets";

/// The directory under a crate's manifest directory that holds its bundle in
/// the source tree.
const SOURCE_DIST_SUBDIR: &str = "dist";

/// Environment variable through which `topcoat dev` hands the fully
/// qualified path of the bundle directory to the application it starts.
const ASSETS_PATH_ENV: &str = "TOPCOAT_ASSETS_PATH";

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
    /// Under `topcoat dev`, the dev server bundles to the owning crate's
    /// `dist/assets` and starts the application with `TOPCOAT_ASSETS_PATH`
    /// naming that directory, which [`load`](Self::load) then loads exactly.
    ///
    /// Without the variable, the bundle is expected in the [`ASSETS_SUBDIR`]
    /// directory next to the executable, where the deploy ships it. A binary
    /// run in its source tree resolves the bundle from its asset records:
    /// the records name their declaring crates, and `dist/assets` next to
    /// one of those directories is loaded, which is how a plain `cargo run`
    /// finds it.
    ///
    /// Use [`load_dir`](Self::load_dir) for a custom location.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::NotFound`] when the resolved location holds
    /// no bundle manifest. Other errors can occur when locating the
    /// executable or loading the manifest through [`load_dir`](Self::load_dir).
    pub fn load() -> io::Result<Self> {
        if let Some(dir) = std::env::var_os(ASSETS_PATH_ENV) {
            return Self::load_env_dir(dir);
        }
        Self::load_at(&std::env::current_exe()?)
    }

    /// Resolves the directory named through `TOPCOAT_ASSETS_PATH`.
    fn load_env_dir(dir: OsString) -> io::Result<Self> {
        let dir = PathBuf::from(dir);
        if dir.join(MANIFEST_NAME).is_file() {
            return Self::load_dir(dir);
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no asset bundle at {} named by {ASSETS_PATH_ENV} \
                 (rebuild under `topcoat dev`, or run `topcoat asset bundle`)",
                dir.display()
            ),
        ))
    }

    /// The resolution behind [`load`](Self::load) for the executable at
    /// `exe`, as if `TOPCOAT_ASSETS_PATH` were unset.
    fn load_at(exe: &Path) -> io::Result<Self> {
        let exe_dir = exe.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "current executable has no parent directory",
            )
        })?;

        let assets_dir = exe_dir.join(ASSETS_SUBDIR);
        if assets_dir.join(MANIFEST_NAME).is_file() {
            return Self::load_dir(assets_dir);
        }

        let binary = fs::read(exe)?;
        let mut seen = HashSet::new();
        for asset in RawAsset::find_in_binary(&binary) {
            if seen.insert(asset.manifest_dir().to_owned()) {
                let dir = Path::new(asset.manifest_dir())
                    .join(SOURCE_DIST_SUBDIR)
                    .join(ASSETS_SUBDIR);
                if dir.join(MANIFEST_NAME).is_file() {
                    return Self::load_dir(dir);
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no asset bundle at {} (ship one there with `topcoat asset bundle`, \
                 or run the application under `topcoat dev`)",
                assets_dir.display()
            ),
        ))
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
    use std::{env, fs};

    use super::*;
    use crate::{AssetOptions, ENCODED_ASSET_SIZE};

    /// A fresh directory under the system temp directory.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("topcoat-asset-bundle-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Writes a minimal valid bundle manifest into `dir`.
    fn write_bundle(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(MANIFEST_NAME), "version = 1\nassets = []\n").unwrap();
    }

    /// An asset record as `asset!` embeds it, naming `manifest_dir`.
    fn record(manifest_dir: &Path) -> [u8; ENCODED_ASSET_SIZE] {
        RawAsset::encode(
            AssetId::new("test", "src/lib.rs", "./icon.png", &AssetOptions::NONE),
            "./icon.png",
            "test",
            manifest_dir.to_str().unwrap(),
            "src/lib.rs",
            &AssetOptions::NONE,
        )
    }

    #[test]
    fn load_prefers_the_assets_beside_the_executable() {
        let root = temp_dir("beside-exe");
        let crate_dir = root.join("crate");
        write_bundle(&root.join(ASSETS_SUBDIR));
        write_bundle(&crate_dir.join(SOURCE_DIST_SUBDIR).join(ASSETS_SUBDIR));
        let exe = root.join("app");
        fs::write(&exe, record(&crate_dir)).unwrap();

        let loaded = AssetBundle::load_at(&exe).unwrap();
        assert_eq!(loaded.dir(), root.join(ASSETS_SUBDIR));
    }

    #[test]
    fn load_finds_the_bundle_next_to_the_recording_crate() {
        let root = temp_dir("records");
        let crate_dir = root.join("crate");
        let bundle = crate_dir.join(SOURCE_DIST_SUBDIR).join(ASSETS_SUBDIR);
        write_bundle(&bundle);
        let exe = root.join("app");
        fs::write(&exe, record(&crate_dir)).unwrap();

        let loaded = AssetBundle::load_at(&exe).unwrap();
        assert_eq!(loaded.dir(), bundle);
    }

    #[test]
    fn load_skips_crates_without_a_bundle() {
        let root = temp_dir("skip");
        fs::create_dir_all(root.join("other").join("src")).unwrap();
        let exe = root.join("app");
        fs::write(&exe, record(&root.join("other"))).unwrap();

        let error = AssetBundle::load_at(&exe).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn load_fails_for_a_binary_without_records() {
        let root = temp_dir("no-records");
        let exe = root.join("app");
        fs::write(&exe, b"").unwrap();

        let error = AssetBundle::load_at(&exe).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn load_env_dir_loads_the_named_directory() {
        let root = temp_dir("env");
        let bundle = root.join("dist").join(ASSETS_SUBDIR);
        write_bundle(&bundle);

        let loaded = AssetBundle::load_env_dir(bundle.clone().into_os_string()).unwrap();
        assert_eq!(loaded.dir(), bundle);
    }

    #[test]
    fn load_env_dir_rejects_a_missing_bundle() {
        let root = temp_dir("env-missing");
        let bundle = root.join("dist").join(ASSETS_SUBDIR);
        fs::create_dir_all(&bundle).unwrap();

        let error = AssetBundle::load_env_dir(bundle.into_os_string()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
