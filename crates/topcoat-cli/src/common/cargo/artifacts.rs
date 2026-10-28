use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::build::BuiltTarget;

/// A Cargo artifact message describing a compiled target and its output files.
#[derive(Deserialize)]
pub(super) struct Artifact {
    package_id: Option<String>,
    target: Target,
    executable: Option<PathBuf>,
    filenames: Vec<PathBuf>,
}

impl Artifact {
    /// Returns the target's final executable or dynamic libraries as built
    /// targets. Skips intermediate dependencies and build-script outputs.
    pub(super) fn built_targets(&self) -> Vec<BuiltTarget> {
        let package = self.package_name().map(str::to_owned);
        if let Some(executable) = &self.executable {
            return vec![BuiltTarget {
                path: executable.clone(),
                package: package.clone(),
            }];
        }
        if !self.is_dynamic_library() {
            return Vec::new();
        }
        self.filenames
            .iter()
            .filter(|filename| is_final_library(filename))
            .map(|path| BuiltTarget {
                path: path.clone(),
                package: package.clone(),
            })
            .collect()
    }

    /// The name of the package this artifact was built for, from the
    /// `package_id` fragment Cargo reports.
    fn package_name(&self) -> Option<&str> {
        let id = self.package_id.as_deref()?;
        let (_, name) = id.split_once('#')?;
        name.split_once('@').map(|(name, _)| name)
    }

    /// Whether the target produces a `cdylib` or `dylib`. Proc-macro libraries are
    /// excluded.
    fn is_dynamic_library(&self) -> bool {
        self.target
            .crate_types
            .iter()
            .any(|ty| ty == "cdylib" || ty == "dylib")
    }
}

#[derive(Deserialize)]
struct Target {
    crate_types: Vec<String>,
}

/// Checks whether a path belongs to a build script's output directory.
fn is_build_script_output(path: &Path) -> bool {
    path.ancestors()
        .any(|a| a.file_name() == Some(OsStr::new("out")))
        && path
            .ancestors()
            .any(|a| a.file_name() == Some(OsStr::new("build")))
}

/// Whether `path` is a final dynamic library outside dependency and build-script output
/// directories.
fn is_final_library(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("wasm" | "so" | "dylib" | "dll")
    ) && path
        .parent()
        .and_then(Path::file_name)
        .is_none_or(|dir| dir != "deps")
        && !is_build_script_output(path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    fn artifact(crate_types: &[&str], filenames: &[&str]) -> Artifact {
        serde_json::from_value(json!({
            "target": { "crate_types": crate_types },
            "executable": null,
            "filenames": filenames,
        }))
        .unwrap()
    }

    fn outputs(artifact: Artifact) -> Vec<PathBuf> {
        artifact
            .built_targets()
            .into_iter()
            .map(|target| target.path)
            .collect()
    }

    #[test]
    fn cdylib_output_is_picked_over_its_rlib_companion() {
        assert_eq!(
            outputs(artifact(
                &["cdylib", "rlib"],
                &["/target/wasm/app.wasm", "/target/wasm/libapp.rlib"],
            )),
            vec![PathBuf::from("/target/wasm/app.wasm")]
        );
    }

    #[test]
    fn artifacts_report_their_owning_package() {
        let artifact: Artifact = serde_json::from_value(json!({
            "package_id": "path+file:///w/apps#web@0.1.0",
            "target": { "crate_types": ["bin"] },
            "executable": "/target/debug/web",
            "filenames": ["/target/debug/web"],
        }))
        .unwrap();

        assert_eq!(
            artifact.built_targets().pop().unwrap().package.as_deref(),
            Some("web")
        );
    }

    #[test]
    fn dependency_cdylib_outputs_are_excluded() {
        // A dependency that declares `cdylib` produces a scannable output
        // too, but cargo leaves it in `deps/`; only the uplifted output of
        // the crate being built belongs in the bundle.
        assert!(
            outputs(artifact(
                &["cdylib", "rlib"],
                &["/target/deps/libdep.dylib"]
            ))
            .is_empty()
        );
    }

    #[test]
    fn proc_macro_libraries_are_not_scanned() {
        // A workspace build uplifts the proc-macro members' shared objects
        // right next to the application's output.
        assert!(outputs(artifact(&["proc-macro"], &["/target/libmacros.dylib"])).is_empty());
    }

    #[test]
    fn plain_library_dependencies_are_not_scanned() {
        assert!(outputs(artifact(&["lib"], &["/target/deps/libdep.rlib"])).is_empty());
    }

    #[test]
    fn recognizes_final_libraries() {
        assert!(is_final_library(Path::new(
            "/t/wasm32-unknown-unknown/debug/app.wasm"
        )));
        assert!(is_final_library(Path::new("/t/debug/libapp.so")));
        assert!(is_final_library(Path::new("/t/debug/libapp.dylib")));
        assert!(is_final_library(Path::new("/t/debug/app.dll")));
        assert!(!is_final_library(Path::new("/t/debug/deps/app.wasm")));
        assert!(!is_final_library(Path::new("/t/debug/deps/libapp.dylib")));
        assert!(!is_final_library(Path::new("/t/debug/libapp.rlib")));
        assert!(!is_final_library(Path::new("/t/debug/app.dll.lib")));
        assert!(!is_final_library(Path::new("/t/debug/app.d")));
    }

    #[test]
    fn build_script_library_outputs_are_excluded() {
        assert!(!is_final_library(Path::new(
            "/t/debug/build/foo/9251c15296da3db3/out/libfoo.so"
        )));
        assert!(!is_final_library(Path::new(
            "/t/debug/build/bar/a3538d1a8934c29c/out/libbar.dylib"
        )));
        assert!(!is_final_library(Path::new(
            "/t/debug/build/foo/extra/nested/path/out/libfoo.so"
        )));
    }

    #[test]
    fn final_library_paths_are_not_excluded() {
        assert!(is_final_library(Path::new("/t/debug/libfoo.so")));
        assert!(is_final_library(Path::new("/t/debug/libfoo.dylib")));
        assert!(is_final_library(Path::new("/t/debug/foo.dll")));
        assert!(is_final_library(Path::new("/t/debug/foo.wasm")));
    }
}
