use std::{
    env,
    fmt::Display,
    fs,
    io::Read as _,
    path::{Path, PathBuf},
};

use crate::iconify::{BuildError, IconSet, Result, set::STAGE_DIR};

/// Stages Iconify icon sets for use at compile time. Call it from a build script:
///
/// ```rust,no_run
/// topcoat::icon::iconify::BuildConfig::new()
///     .icon_set("lucide")
///     .icon_set_version("mdi", "1.30.0")
///     .stage()
///     .unwrap();
/// ```
///
/// Downloads missing sets from jsDelivr and stages them under `OUT_DIR`.
/// Cached sets are reused without network access.
///
/// The default cache is shared across workspace builds. Use
/// [`cache_dir`](Self::cache_dir) to choose a directory or supply your own sets.
#[derive(Debug, Default)]
pub struct BuildConfig {
    cache_dir: Option<PathBuf>,
    sets: Vec<Set>,
}

impl BuildConfig {
    /// Creates a configuration without any sets.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stages the latest version of `set`, downloading it when it is not
    /// cached yet. To pick up new releases, delete the cached copy or pin a
    /// version with [`icon_set_version`](Self::icon_set_version).
    #[must_use]
    pub fn icon_set(mut self, set: impl Into<String>) -> Self {
        self.sets.push(Set {
            name: set.into(),
            version: None,
        });
        self
    }

    /// Stages `version` of `set`. Downloads it if the cache has no copy of
    /// that version yet.
    #[must_use]
    pub fn icon_set_version(mut self, set: impl Into<String>, version: impl Into<String>) -> Self {
        self.sets.push(Set {
            name: set.into(),
            version: Some(version.into()),
        });
        self
    }

    /// Caches downloaded sets in `dir` instead of the shared Topcoat cache
    /// under the user's cache directory. Relative paths resolve against
    /// `CARGO_MANIFEST_DIR` (the package root).
    ///
    /// Each set is cached at `<dir>/<set>/<version>.json`, with unpinned
    /// sets under `latest`, and downloaded only when its file is missing.
    /// Files you place there yourself are used as-is. Commit the directory
    /// for offline, reproducible builds, or gitignore it to keep a cache
    /// that survives `cargo clean`.
    #[must_use]
    pub fn cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(dir.into());
        self
    }

    /// Stages every set into `$OUT_DIR/topcoat-icon-iconify`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `OUT_DIR` is unset (or `CARGO_MANIFEST_DIR`, with a
    /// cache directory), a download fails, a file cannot be read or written,
    /// or a set does not match the `IconifyJSON` schema, declares a prefix
    /// other than the name it is staged as, or contains an alias that does
    /// not lead to an icon.
    pub fn stage(self) -> Result {
        let out_dir = env::var_os("OUT_DIR").ok_or(BuildError::NoOutDir)?;
        let dir = PathBuf::from(out_dir).join(STAGE_DIR);
        fs::create_dir_all(&dir).map_err(|source| BuildError::Io {
            path: dir.clone(),
            source,
        })?;

        let cache_dir = match self.cache_dir {
            Some(cache_dir) => {
                let manifest_dir =
                    env::var_os("CARGO_MANIFEST_DIR").ok_or(BuildError::NoManifestDir)?;
                Some(PathBuf::from(manifest_dir).join(cache_dir))
            }
            // Without a home directory there is no shared cache; cache inside
            // the stage directory's own `cache` folder instead.
            None => topcoat_core::cache::user_cache_dir("iconify"),
        };
        let cache_dir = cache_dir.unwrap_or_else(|| dir.join("cache"));

        for set in self.sets {
            set.stage(&dir, &cache_dir)?;
        }

        Ok(())
    }
}

/// The cache key unpinned sets are staged under.
const LATEST: &str = "latest";

/// One icon set to stage: the name of its `@iconify-json/<name>` package and
/// an optionally pinned version.
#[derive(Debug)]
struct Set {
    name: String,
    version: Option<String>,
}

impl Set {
    /// Stages this set into `dir`, downloading it through the cache at
    /// `cache_dir` when the cache has no copy of the requested version yet.
    ///
    /// The cache keeps one file per version at `<cache>/<set>/<version>.json`,
    /// with unpinned sets under [`LATEST`], so concurrent builds publish
    /// different versions independently and every file appears complete.
    fn stage(self, dir: &Path, cache_dir: &Path) -> Result {
        let version = self.version.as_deref().unwrap_or(LATEST);
        let cached = cache_dir.join(&self.name).join(format!("{version}.json"));

        if !cached.exists() {
            let bytes = self.fetch()?;
            let set_dir = cached.parent().expect("cached path has a parent");
            fs::create_dir_all(set_dir).map_err(|source| BuildError::Io {
                path: set_dir.to_path_buf(),
                source,
            })?;
            publish(&cached, &bytes)?;
        }

        let bytes = fs::read(&cached).map_err(|source| BuildError::Io {
            path: cached.clone(),
            source,
        })?;
        self.check_prefix(&parse(&bytes, cached.display())?)?;
        publish(&dir.join(format!("{}.json", self.name)), &bytes)
    }

    /// Fetches this set from jsDelivr, at the pinned version or the `latest`
    /// tag, checking that the payload parses and declares the requested
    /// prefix.
    fn fetch(&self) -> Result<Vec<u8>> {
        let tag = self.version.as_deref().unwrap_or("latest");
        let url = format!(
            "https://cdn.jsdelivr.net/npm/@iconify-json/{name}@{tag}/icons.json",
            name = self.name,
        );
        let bytes = download(&url)?;
        self.check_prefix(&parse(&bytes, &url)?)?;
        Ok(bytes)
    }

    /// Checks that a cached or downloaded set declares the prefix it is
    /// staged as, which is also the name its icons are addressed by.
    fn check_prefix(&self, parsed: &IconSet) -> Result {
        if parsed.prefix != self.name {
            return Err(BuildError::PrefixMismatch {
                requested: self.name.clone(),
                declared: parsed.prefix.clone(),
            });
        }
        Ok(())
    }
}

/// Fetches `url` into memory.
fn download(url: &str) -> Result<Vec<u8>> {
    let http_error = |source: ureq::Error| BuildError::Http {
        url: url.to_owned(),
        source: Box::new(source),
    };

    let mut body = ureq::get(url).call().map_err(http_error)?.into_body();
    let mut bytes = Vec::new();
    body.as_reader()
        .read_to_end(&mut bytes)
        .map_err(|source| http_error(source.into()))?;
    Ok(bytes)
}

/// Parses and checks a set: the JSON must match the `IconifyJSON` schema and
/// every alias must lead to an icon, so problems surface while staging
/// rather than in a macro expansion.
fn parse(bytes: &[u8], origin: impl Display) -> Result<IconSet> {
    let set: IconSet = serde_json::from_slice(bytes).map_err(|source| BuildError::Json {
        origin: origin.to_string(),
        source,
    })?;

    for (name, alias) in &set.aliases {
        let mut parent = &alias.parent;
        let mut steps = 0;
        while !set.icons.contains_key(parent) {
            let Some(next) = set.aliases.get(parent) else {
                return Err(BuildError::UnknownAliasParent {
                    prefix: set.prefix.clone(),
                    alias: name.clone(),
                    parent: parent.clone(),
                });
            };
            steps += 1;
            if steps > set.aliases.len() {
                return Err(BuildError::AliasCycle {
                    prefix: set.prefix.clone(),
                    alias: name.clone(),
                });
            }
            parent = &next.parent;
        }
    }

    Ok(set)
}

/// Publishes `bytes` at `path` atomically: writes a temporary file next to
/// it and renames it into place, so concurrent readers never see a partial
/// file and concurrent writers of the same version never interleave.
fn publish(path: &Path, bytes: &[u8]) -> Result {
    let file = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let temp = path.with_file_name(format!("{file}.tmp-{}", std::process::id()));

    fs::write(&temp, bytes).map_err(|source| BuildError::Io {
        path: temp.clone(),
        source,
    })?;
    if let Err(source) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(BuildError::Io {
            path: path.to_path_buf(),
            source,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system temp directory.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("topcoat-icon-iconify-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn set(name: &str, version: Option<&str>) -> Set {
        Set {
            name: name.to_owned(),
            version: version.map(str::to_owned),
        }
    }

    const DEMO: &[u8] = br#"{ "prefix": "demo", "icons": { "trash": { "body": "<g/>" } } }"#;
    const DEMO_TWO: &[u8] = br#"{ "prefix": "demo", "icons": { "bin": { "body": "<g/>" } } }"#;

    #[test]
    fn cached_files_stage_offline() {
        let root = temp_dir("cached-files-stage");
        let cache_dir = root.join("icons");
        let stage_dir = root.join("staged");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::create_dir_all(&stage_dir).unwrap();

        // A fetch of the nonexistent `demo` package would fail.
        fs::create_dir_all(cache_dir.join("demo")).unwrap();
        fs::write(cache_dir.join("demo").join("latest.json"), DEMO).unwrap();
        set("demo", None).stage(&stage_dir, &cache_dir).unwrap();

        assert_eq!(fs::read(stage_dir.join("demo.json")).unwrap(), DEMO);
    }

    #[test]
    fn pinned_sets_reuse_copies_of_the_same_version() {
        let root = temp_dir("pinned-sets-reuse");
        let cache_dir = root.join("icons");
        let stage_dir = root.join("staged");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::create_dir_all(&stage_dir).unwrap();

        fs::create_dir_all(cache_dir.join("demo")).unwrap();
        fs::write(cache_dir.join("demo").join("9.9.9.json"), DEMO).unwrap();
        set("demo", Some("9.9.9"))
            .stage(&stage_dir, &cache_dir)
            .unwrap();

        assert_eq!(fs::read(stage_dir.join("demo.json")).unwrap(), DEMO);
    }

    #[test]
    fn pinned_versions_are_cached_independently() {
        let root = temp_dir("pinned-versions-independent");
        let cache_dir = root.join("icons");
        let stage_dir = root.join("staged");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::create_dir_all(&stage_dir).unwrap();

        fs::create_dir_all(cache_dir.join("demo")).unwrap();
        fs::write(cache_dir.join("demo").join("1.0.0.json"), DEMO).unwrap();
        fs::write(cache_dir.join("demo").join("2.0.0.json"), DEMO_TWO).unwrap();

        set("demo", Some("1.0.0"))
            .stage(&stage_dir, &cache_dir)
            .unwrap();
        assert_eq!(fs::read(stage_dir.join("demo.json")).unwrap(), DEMO);

        set("demo", Some("2.0.0"))
            .stage(&stage_dir, &cache_dir)
            .unwrap();
        assert_eq!(fs::read(stage_dir.join("demo.json")).unwrap(), DEMO_TWO);

        // Both versions stay cached; staging one does not evict the other.
        assert!(cache_dir.join("demo").join("1.0.0.json").is_file());
        assert!(cache_dir.join("demo").join("2.0.0.json").is_file());
    }

    #[test]
    fn the_build_dir_cache_reuses_files_as_they_are() {
        let root = temp_dir("build-dir-reuse");
        let stage_dir = root.join("staged");
        fs::create_dir_all(stage_dir.join("cache").join("demo")).unwrap();

        fs::write(
            stage_dir.join("cache").join("demo").join("latest.json"),
            DEMO,
        )
        .unwrap();
        set("demo", None)
            .stage(&stage_dir, &stage_dir.join("cache"))
            .unwrap();

        assert_eq!(fs::read(stage_dir.join("demo.json")).unwrap(), DEMO);
    }

    #[test]
    fn cached_files_must_declare_their_prefix() {
        let root = temp_dir("cached-files-prefix");
        let cache_dir = root.join("icons");
        let stage_dir = root.join("staged");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::create_dir_all(&stage_dir).unwrap();

        let json = br#"{ "prefix": "other", "icons": { "trash": { "body": "<g/>" } } }"#;
        fs::create_dir_all(cache_dir.join("demo")).unwrap();
        fs::write(cache_dir.join("demo").join("latest.json"), json).unwrap();

        let error = set("demo", None).stage(&stage_dir, &cache_dir).unwrap_err();
        assert!(
            matches!(
                &error,
                BuildError::PrefixMismatch { requested, declared }
                    if requested == "demo" && declared == "other"
            ),
            "{error}"
        );
    }

    #[test]
    fn parse_reports_dangling_alias_parents() {
        let json = br#"{
            "prefix": "demo",
            "icons": { "trash": { "body": "<g/>" } },
            "aliases": { "bin": { "parent": "rubbish" } }
        }"#;

        let error = parse(json, "test").unwrap_err();
        assert!(
            matches!(
                &error,
                BuildError::UnknownAliasParent { prefix, alias, parent }
                    if prefix == "demo" && alias == "bin" && parent == "rubbish"
            ),
            "{error}"
        );
    }

    #[test]
    fn parse_reports_alias_cycles() {
        let json = br#"{
            "prefix": "demo",
            "icons": { "trash": { "body": "<g/>" } },
            "aliases": {
                "yin": { "parent": "yang" },
                "yang": { "parent": "yin" }
            }
        }"#;

        let error = parse(json, "test").unwrap_err();
        assert!(
            matches!(&error, BuildError::AliasCycle { prefix, .. } if prefix == "demo"),
            "{error}"
        );
    }

    #[test]
    fn parse_accepts_alias_chains() {
        let json = br#"{
            "prefix": "demo",
            "icons": { "trash": { "body": "<g/>" } },
            "aliases": {
                "bin": { "parent": "trash" },
                "wastebasket": { "parent": "bin" }
            }
        }"#;

        assert!(parse(json, "test").is_ok());
    }
}
