use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static NATIVE_ARTIFACT: OnceLock<PathBuf> = OnceLock::new();

/// Removes inherited access from a staged module directory on Windows.
///
/// The native loader rejects module directories writable by other users, so
/// test fixtures must grant access only to the current runner account.
pub fn secure_fixture_directory(path: &Path) {
    #[cfg(windows)]
    {
        let identity = std::process::Command::new("powershell")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value",
            ])
            .output()
            .expect("PowerShell is available on Windows test runners");
        assert!(
            identity.status.success(),
            "current-user SID query failed: {identity:?}"
        );
        let sid = String::from_utf8(identity.stdout).expect("SID output is UTF-8");
        let grant = format!("*{}:(OI)(CI)F", sid.trim());
        let acl = std::process::Command::new("icacls")
            .arg(path)
            .args(["/inheritance:r", "/grant:r"])
            .arg(grant)
            .output()
            .expect("icacls is available on Windows test runners");
        assert!(acl.status.success(), "private fixture ACL failed: {acl:?}");
    }

    #[cfg(not(windows))]
    let _ = path;
}

pub fn artifact() -> PathBuf {
    if let Some(path) = std::env::var_os("TINYMEMES_MODULE") {
        return PathBuf::from(path);
    }
    NATIVE_ARTIFACT.get_or_init(build_dynamic_artifact).clone()
}

fn build_dynamic_artifact() -> PathBuf {
    let exe = std::env::current_exe().expect("current test executable");
    let profile_dir = exe.parent().and_then(Path::parent).unwrap().to_owned();
    let root = exe
        .ancestors()
        .find(|ancestor| {
            ancestor
                .join("crates/tinymemes-module/Cargo.toml")
                .is_file()
        })
        .expect("find the TinyMemes checkout from the test executable");
    let name = format!(
        "{}tinymemes_module{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    let manifest = root.join("crates/tinymemes-module/Cargo.toml");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut build = std::process::Command::new(cargo);
    build.current_dir(root).args([
        "build",
        "--locked",
        "--manifest-path",
        manifest.to_str().expect("manifest path is UTF-8"),
        "--package",
        "tinymemes-module",
        "--no-default-features",
        "--message-format=json-render-diagnostics",
    ]);
    if profile_dir.ends_with("release") {
        build.arg("--release");
    }
    let output = build.output().expect("start the dynamic module build");
    assert!(
        output.status.success(),
        "build the dynamic module: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("Cargo JSON is UTF-8");
    let source = root.join("crates/tinymemes-module/src/lib.rs");
    let artifact = select_dynamic_module_artifact(&stdout, &source, &name)
        .expect("Cargo reported the dynamic module artifact");
    assert!(
        artifact.is_file(),
        "Cargo reported missing module {artifact:?}"
    );
    artifact
}

pub fn select_dynamic_module_artifact(
    output: &str,
    expected_source: &Path,
    library_name: &str,
) -> Result<PathBuf, String> {
    let expected_source_identity = std::fs::canonicalize(expected_source).map_err(|error| {
        format!(
            "could not resolve expected module source {:?}: {error}",
            expected_source
        )
    })?;
    let mut artifact = None;
    let mut candidate_diagnostics = Vec::new();
    for line in output.lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if message["reason"] != "compiler-artifact" {
            continue;
        }
        let target = &message["target"];
        let target_name = target["name"].as_str();
        let target_source = target["src_path"].as_str();
        let target_crate_types = target["crate_types"].as_array();
        let features = message["features"].as_array();
        let filenames = message["filenames"].as_array();
        let source_looks_owned = target_source.is_some_and(|source| {
            source
                .replace('\\', "/")
                .to_ascii_lowercase()
                .ends_with("/crates/tinymemes-module/src/lib.rs")
        });
        if (target_name == Some("tinymemes_module") || source_looks_owned)
            && candidate_diagnostics.len() < 3
        {
            candidate_diagnostics.push(format!(
                "reason=compiler-artifact target.name={:?} target.src_path={:?} \
                 target.crate_types={:?} features={:?} filenames={:?}",
                target_name, target_source, target_crate_types, features, filenames,
            ));
        }

        let source_matches = target_source
            .and_then(|source| std::fs::canonicalize(source).ok())
            .is_some_and(|source| source == expected_source_identity);
        if !source_matches
            || target_name != Some("tinymemes_module")
            || !message["target"]["crate_types"]
                .as_array()
                .is_some_and(|types| types.iter().any(|kind| kind == "cdylib"))
        {
            continue;
        }
        let features = message["features"]
            .as_array()
            .ok_or_else(|| "Cargo artifact features are not an array".to_owned())?;
        if features
            .iter()
            .any(|feature| feature.as_str() == Some("static-link"))
        {
            return Err(
                "Cargo selected the static-link artifact for the native loader test".into(),
            );
        }
        for filename in message["filenames"]
            .as_array()
            .ok_or_else(|| "Cargo artifact filenames are not an array".to_owned())?
        {
            let Some(filename) = filename.as_str() else {
                continue;
            };
            let candidate = PathBuf::from(filename);
            if candidate.file_name().and_then(|name| name.to_str()) == Some(library_name) {
                artifact = Some(candidate);
            }
        }
    }
    artifact.ok_or_else(|| {
        let candidates = if candidate_diagnostics.is_empty() {
            "no compiler-artifact target named tinymemes_module".to_owned()
        } else {
            candidate_diagnostics.join("; ")
        };
        format!("Cargo did not report the dynamic module artifact; candidates: {candidates}")
    })
}
