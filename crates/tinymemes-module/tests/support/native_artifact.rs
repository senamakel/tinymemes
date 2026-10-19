use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static NATIVE_ARTIFACT: OnceLock<PathBuf> = OnceLock::new();

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
    let mut artifact = None;
    for line in output.lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if message["reason"] != "compiler-artifact"
            || message["target"]["src_path"] != expected_source.to_string_lossy().as_ref()
            || message["target"]["name"] != "tinymemes_module"
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
    artifact.ok_or_else(|| "Cargo did not report the dynamic module artifact".to_owned())
}
