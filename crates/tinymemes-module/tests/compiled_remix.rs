//! Drives the real compiled `cdylib` through the module loader: a scripted
//! host serves the callback interface and the module remixes a reply over the
//! bus, falling back to the chat model for Jev because the host has none.
//!
//! The artifact is built with `cargo build -p tinymemes-module` unless
//! `TINYMEMES_MODULE` points at one (the path override CI and hosts use).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tinybus::broker::Broker;
use tinybus::module::ModuleHost;
use tinybus::transport::memory::MemoryBus;
use tinybus::{CallContext, Connection, InterfaceName, MemberName, ObjectPath};
use tinymemes_bus::{
    ApplyRequest, ContentOperation, ContentState, HOST_INTERFACE, HOST_METHODS, HOST_PATH,
    HostFailure, RemixRequest, host_methods,
};
use tinymemes_client::{RemixClient, StateClient};

#[path = "support/native_artifact.rs"]
mod native_artifact;

fn artifact() -> PathBuf {
    native_artifact::artifact()
}

/// One callback as the host saw it.
#[derive(Clone, Debug)]
struct Seen {
    member: String,
    correlation: String,
    sender_is_authenticated: bool,
}

struct ScriptedHost {
    seen: Arc<Mutex<Vec<Seen>>>,
}

#[async_trait]
impl tinybus::Interface for ScriptedHost {
    fn name(&self) -> InterfaceName {
        InterfaceName::new(HOST_INTERFACE).unwrap()
    }
    fn members(&self) -> Vec<MemberName> {
        HOST_METHODS
            .iter()
            .map(|m| MemberName::new(*m).unwrap())
            .collect()
    }
    async fn call(
        &self,
        _: &MemberName,
        _: serde_json::Value,
    ) -> tinybus::Result<serde_json::Value> {
        Err(tinybus::Error::failed("unauthenticated"))
    }
    async fn call_with_context(
        &self,
        member: &MemberName,
        args: serde_json::Value,
        context: &CallContext,
    ) -> tinybus::Result<serde_json::Value> {
        let (payload,): (serde_json::Value,) = serde_json::from_value(args).unwrap();
        self.seen.lock().unwrap().push(Seen {
            member: member.as_str().to_owned(),
            correlation: payload["correlation"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            sender_is_authenticated: context.authenticated_sender().is_some(),
        });
        let reply = match member.as_str() {
            // No Jev here: the module must ask the chat model instead.
            host_methods::EVALUATE | host_methods::SEARCH => {
                serde_json::to_value(Result::<(), _>::Err(HostFailure::Unsupported))
            }
            host_methods::COMPLETE => {
                let text = if payload["system"]
                    .as_str()
                    .unwrap_or("")
                    .contains("classification")
                {
                    r#"{"chat_intent": {"choice": "grind"}, "reply_intent": {"choice": "grind"},
                        "frankness": {"level": 2}, "playful": {"p": 0.2}, "serious": {"p": 0.0}}"#
                } else {
                    "ngl that's done"
                };
                serde_json::to_value(Result::<_, HostFailure>::Ok(text))
            }
            host_methods::LOAD_INDEX => {
                serde_json::to_value(Result::<Option<String>, HostFailure>::Ok(None))
            }
            _ => serde_json::to_value(Result::<(), HostFailure>::Ok(())),
        };
        Ok(reply.unwrap())
    }
}

async fn load() -> (
    ModuleHost,
    Connection,
    tokio::task::JoinHandle<tinybus::Result<()>>,
    tempfile_dir::Dir,
) {
    let artifact = artifact();
    let hash = tinybus::module::sha256_file(&artifact).unwrap();
    let dir = tempfile_dir::Dir::new_in(artifact.parent().unwrap());
    let staged = dir.path().join(artifact.file_name().unwrap());
    std::fs::copy(&artifact, &staged).unwrap();
    std::fs::write(
        dir.path().join("modules.toml"),
        format!(
            "{:?} = {hash:?}\n",
            staged.file_name().unwrap().to_str().unwrap()
        ),
    )
    .unwrap();
    let bus = MemoryBus::new();
    let broker = Broker::new();
    let task = broker.spawn(bus.clone());
    let host = ModuleHost::new(broker);
    let info = host.load_file(&staged).unwrap();
    assert_eq!(info.name, env!("CARGO_PKG_NAME"));
    let caller = Connection::connect(bus.connect().await.unwrap())
        .await
        .unwrap();
    (host, caller, task, dir)
}

/// A scratch directory next to the artifact, removed on drop.
mod tempfile_dir {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    #[cfg(windows)]
    fn secure_windows_fixture_directory(path: &Path) {
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

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    pub struct Dir(PathBuf);
    impl Dir {
        pub fn new_in(parent: &Path) -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("compiled-remix-{}-{serial}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            #[cfg(windows)]
            secure_windows_fixture_directory(&path);
            Self(path)
        }
        pub fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[tokio::test]
async fn the_public_loader_rejects_digest_pinned_invalid_native_bytes() {
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let dir = tempfile_dir::Dir::new_in(&target);
    let library = format!(
        "{}tinymemes_module{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    let artifact = dir.path().join(&library);
    std::fs::write(&artifact, b"not a TinyBus native library").unwrap();
    let digest = tinybus::module::sha256_file(&artifact).unwrap();
    std::fs::write(
        dir.path().join("modules.toml"),
        format!("{library:?} = {digest:?}\n"),
    )
    .unwrap();

    let bus = MemoryBus::new();
    let broker = Broker::new();
    let _task = broker.spawn(bus);
    let error = match ModuleHost::new(broker).load_file(&artifact) {
        Err(error) => error,
        Ok(_) => panic!("the public loader accepted digest-pinned invalid native bytes"),
    };
    assert!(
        error
            .to_string()
            .contains("dynamic loader rejected the artifact"),
        "the negative control must reach the native loader, not fail an earlier gate: {error}"
    );
}

fn request(correlation: &str) -> RemixRequest {
    serde_json::from_value(serde_json::json!({
        "conversation": [{"role": "user", "text": "ok the report is done", "id": "u-1"}],
        "reply": "That's done.",
        "region": "global",
        "correlation": correlation,
        "reply_id": "run-1:reply",
        "timeout_ms": 20_000,
    }))
    .unwrap()
}

#[test]
fn cargo_artifact_selection_accepts_a_real_source_path_alias() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let alias_source = PathBuf::from(format!(
        "{}{sep}.{sep}lib.rs",
        source.parent().unwrap().display(),
        sep = std::path::MAIN_SEPARATOR,
    ));
    assert!(alias_source.is_file());
    assert_ne!(alias_source.to_string_lossy(), source.to_string_lossy());
    let alias = serde_json::json!({
        "reason": "compiler-artifact",
        "package_id": "path+file:///work/crates/tinymemes-module#0.4.2",
        "target": { "name": "tinymemes_module", "src_path": alias_source, "crate_types": ["cdylib"] },
        "features": [],
        "filenames": ["/build/alias/libtinymemes_module.dylib"]
    });
    assert_eq!(
        native_artifact::select_dynamic_module_artifact(
            &alias.to_string(),
            &source,
            "libtinymemes_module.dylib",
        )
        .unwrap(),
        PathBuf::from("/build/alias/libtinymemes_module.dylib")
    );
}

#[test]
fn cargo_artifact_selection_accepts_compact_local_package_ids_only_for_the_module_source() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let compact = serde_json::json!({
        "reason": "compiler-artifact",
        "package_id": "path+file:///work/crates/tinymemes-module#0.4.2",
        "target": { "name": "tinymemes_module", "src_path": source, "crate_types": ["cdylib"] },
        "features": [],
        "filenames": ["/build/libtinymemes_module.dylib"]
    });
    let expanded = serde_json::json!({
        "reason": "compiler-artifact",
        "package_id": "path+file:///work/crates/tinymemes-module#tinymemes-module@0.4.2",
        "target": { "name": "tinymemes_module", "src_path": source, "crate_types": ["cdylib"] },
        "features": [],
        "filenames": ["/build/expanded/libtinymemes_module.dylib"]
    });
    let foreign = serde_json::json!({
        "reason": "compiler-artifact",
        "package_id": "path+file:///other/crates/tinymemes-module#0.4.2",
        "target": { "name": "tinymemes_module", "src_path": "/other/src/lib.rs", "crate_types": ["cdylib"] },
        "features": [],
        "filenames": ["/build/foreign/libtinymemes_module.dylib"]
    });
    let static_link = serde_json::json!({
        "reason": "compiler-artifact",
        "package_id": "path+file:///work/crates/tinymemes-module#0.4.2",
        "target": { "name": "tinymemes_module", "src_path": source, "crate_types": ["cdylib"] },
        "features": ["static-link"],
        "filenames": ["/build/static/libtinymemes_module.dylib"]
    });
    let non_cdylib = serde_json::json!({
        "reason": "compiler-artifact",
        "package_id": "path+file:///work/crates/tinymemes-module#0.4.2",
        "target": { "name": "tinymemes_module", "src_path": source, "crate_types": ["rlib"] },
        "features": [],
        "filenames": ["/build/rlib/libtinymemes_module.dylib"]
    });
    let compact_output = compact.to_string();
    let foreign_error = native_artifact::select_dynamic_module_artifact(
        &foreign.to_string(),
        &source,
        "libtinymemes_module.dylib",
    )
    .unwrap_err();
    assert!(
        foreign_error.contains("reason=compiler-artifact"),
        "{foreign_error}"
    );
    assert!(
        foreign_error.contains("target.name=Some(\"tinymemes_module\")"),
        "{foreign_error}"
    );
    assert!(
        foreign_error.contains("target.src_path=Some(\"/other/src/lib.rs\")"),
        "{foreign_error}"
    );
    assert!(
        foreign_error.contains("features=Some([])"),
        "{foreign_error}"
    );
    assert!(!foreign_error.contains("package_id"), "{foreign_error}");
    assert_eq!(
        native_artifact::select_dynamic_module_artifact(
            &compact_output,
            &source,
            "libtinymemes_module.dylib",
        )
        .unwrap(),
        PathBuf::from("/build/libtinymemes_module.dylib")
    );
    let output = [foreign, expanded, non_cdylib]
        .iter()
        .map(serde_json::Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        native_artifact::select_dynamic_module_artifact(
            &output,
            &source,
            "libtinymemes_module.dylib",
        )
        .unwrap(),
        PathBuf::from("/build/expanded/libtinymemes_module.dylib")
    );
    let output = [serde_json::from_str(&compact_output).unwrap(), static_link]
        .iter()
        .map(serde_json::Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        native_artifact::select_dynamic_module_artifact(
            &output,
            &source,
            "libtinymemes_module.dylib",
        )
        .unwrap_err()
        .contains("static-link")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_compiled_module_remixes_through_the_hosts_callbacks_with_a_correlation_id() {
    let (_host, caller, _task, _dir) = load().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    caller
        .serve_at(
            ObjectPath::new(HOST_PATH).unwrap(),
            ScriptedHost { seen: seen.clone() },
        )
        .await
        .unwrap();
    let client = RemixClient::new(&caller).unwrap();
    assert!(format!("{client:?}").contains("RemixClient { .. }"));
    let state_client = StateClient::new(&caller).unwrap();
    assert!(format!("{state_client:?}").contains("StateClient { .. }"));
    let applied = state_client
        .apply(ApplyRequest {
            state: ContentState::default(),
            expected_revision: 0,
            operation: ContentOperation::MarkRemixed {
                reply_id: "state-client-reply".into(),
            },
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(applied.revision, 1);
    assert_eq!(applied.remixed, ["state-client-reply"]);
    let mut response = None;
    for _ in 0..50 {
        match client.remix(request("corr-e2e-1")).await {
            Ok(result) => {
                response = Some(result);
                break;
            }
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
        }
    }
    let response = response
        .expect("module answered")
        .expect("module accepted the caller");
    assert!(response.remixed, "{response:?}");
    assert_eq!(response.reply, "ngl that's done");

    let seen = seen.lock().unwrap().clone();
    let members: Vec<&str> = seen.iter().map(|s| s.member.as_str()).collect();
    // The host has no Jev, so the model answered Jev's questions, then rewrote.
    assert!(members.contains(&"Evaluate"), "{members:?}");
    assert!(
        members.iter().filter(|m| **m == "Complete").count() >= 2,
        "{members:?}"
    );
    assert!(members.contains(&"SaveIndex"), "{members:?}");
    assert!(
        seen.iter().all(|s| s.correlation == "corr-e2e-1"),
        "every callback carries the request's correlation id: {seen:?}"
    );
    assert!(seen.iter().all(|s| s.sender_is_authenticated), "{seen:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_remixes_keep_their_own_correlation_ids() {
    let (_host, caller, _task, _dir) = load().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    caller
        .serve_at(
            ObjectPath::new(HOST_PATH).unwrap(),
            ScriptedHost { seen: seen.clone() },
        )
        .await
        .unwrap();
    let client = RemixClient::new(&caller).unwrap();
    // Wait for the module to come up.
    for _ in 0..50 {
        if client.remix(request("warmup")).await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    seen.lock().unwrap().clear();
    let calls = (0..4).map(|i| {
        let client = client.clone();
        tokio::spawn(async move { client.remix(request(&format!("corr-{i}"))).await })
    });
    for call in calls {
        let response = call.await.unwrap().unwrap().unwrap();
        assert!(response.remixed, "{response:?}");
    }
    let seen: Vec<Seen> = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|s| s.correlation != "warmup")
        .cloned()
        .collect();
    for i in 0..4 {
        let id = format!("corr-{i}");
        assert!(
            seen.iter()
                .any(|s| s.member == "Complete" && s.correlation == id),
            "{id} reached the host: {seen:?}"
        );
    }
    assert!(
        seen.iter().all(|s| s.correlation.starts_with("corr-")),
        "{seen:?}"
    );
}
