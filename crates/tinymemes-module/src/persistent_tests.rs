use super::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tinybus::{
    CallContext, Connection, Interface, MemberName, broker::Broker, module::ModuleHost,
    transport::memory::MemoryBus,
};
use tinymemes_bus::{MARKER_PATH, MarkRemixedReply, MarkerOutcome, MarkerRequest};

#[derive(Default)]
struct Documents(Mutex<HashMap<String, tinystoragedrivers_bus::Versioned>>);

#[async_trait::async_trait]
impl Interface for Documents {
    fn name(&self) -> tinybus::InterfaceName {
        tinystoragedrivers_bus::INTERFACE.try_into().unwrap()
    }

    fn members(&self) -> Vec<MemberName> {
        ["Get", "Put"]
            .into_iter()
            .map(|member| MemberName::new(member).unwrap())
            .collect()
    }

    fn requires_confidential(&self, _: &MemberName) -> bool {
        true
    }

    async fn call(
        &self,
        _: &MemberName,
        _: serde_json::Value,
    ) -> tinybus::Result<serde_json::Value> {
        Err(tinybus::Error::failed(
            "confidential document call required",
        ))
    }

    async fn call_with_context(
        &self,
        member: &MemberName,
        args: serde_json::Value,
        context: &CallContext,
    ) -> tinybus::Result<serde_json::Value> {
        if !context.is_confidential() || context.authenticated_sender().is_none() {
            return Err(tinybus::Error::failed(
                "authenticated document call required",
            ));
        }
        match member.as_str() {
            "Get" => {
                let (request,): (tinystoragedrivers_bus::GetRequest,) =
                    serde_json::from_value(args).unwrap();
                let document = self.0.lock().unwrap().get(&request.id).cloned();
                Ok(serde_json::to_value(Ok::<
                    _,
                    tinystoragedrivers_bus::StorageFailure,
                >(document))?)
            }
            "Put" => {
                let (request,): (tinystoragedrivers_bus::PutRequest,) =
                    serde_json::from_value(args).unwrap();
                let mut documents = self.0.lock().unwrap();
                let current = documents.get(&request.id);
                let matches = match request.precondition {
                    tinystoragedrivers_bus::Precondition::None => true,
                    tinystoragedrivers_bus::Precondition::Absent => current.is_none(),
                    tinystoragedrivers_bus::Precondition::Version(version) => {
                        current.is_some_and(|document| document.version == version)
                    }
                };
                if !matches {
                    return Ok(serde_json::to_value(Err::<
                        tinystoragedrivers_bus::Version,
                        _,
                    >(
                        tinystoragedrivers_bus::StorageFailure {
                            kind: tinystoragedrivers_bus::StorageKind::Conflict,
                        },
                    ))?);
                }
                let version = current.map_or(1, |document| document.version.0 + 1);
                let version = tinystoragedrivers_bus::Version(version);
                documents.insert(
                    request.id.clone(),
                    tinystoragedrivers_bus::Versioned {
                        id: request.id,
                        version,
                        doc: request.doc,
                        meta: None,
                    },
                );
                Ok(serde_json::to_value(Ok::<
                    _,
                    tinystoragedrivers_bus::StorageFailure,
                >(version))?)
            }
            _ => Err(tinybus::Error::failed("unknown document method")),
        }
    }
}

struct Fixture {
    _host: ModuleHost,
    module: Connection,
    documents: Arc<Documents>,
    _broker_task: tokio::task::JoinHandle<tinybus::Result<()>>,
}

impl Fixture {
    async fn new() -> Self {
        let bus = MemoryBus::new();
        let broker = Broker::new();
        let broker_task = broker.clone().spawn(bus.clone());
        let host = ModuleHost::new(broker);
        let documents = Arc::new(Documents::default());
        let module_connection = Arc::new(Mutex::new(None));
        let capture = module_connection.clone();
        let module_documents = documents.clone();
        host.attach_linked_with_setup(
            crate::linked_module().expect("linked TinyMemes manifest is valid"),
            serde_json::json!({}),
            move |connection| async move {
                connection
                    .serve_at(
                        MARKER_PATH.try_into().unwrap(),
                        PersistentService {
                            connection: connection.clone(),
                        },
                    )
                    .await?;
                connection
                    .serve_at(MARKER_DOCUMENT_PATH.try_into().unwrap(), module_documents)
                    .await?;
                *capture.lock().unwrap() = Some(connection.clone());
                Ok(())
            },
        )
        .await
        .unwrap();
        let module = module_connection.lock().unwrap().take().unwrap();
        Self {
            _host: host,
            module,
            documents,
            _broker_task: broker_task,
        }
    }

    fn proxy(&self) -> tinybus::Proxy {
        let peer = self.module.unique_name().unwrap();
        self.module
            .proxy(peer.as_str(), MARKER_PATH, MARKER_INTERFACE)
            .unwrap()
    }
}

#[test]
fn every_native_storage_kind_maps_to_its_contract_mirror() {
    let json = |kind: StorageKind| serde_json::to_value(kind).unwrap();
    for kind in [
        StorageKind::NotFound,
        StorageKind::Conflict,
        StorageKind::AlreadyExists,
        StorageKind::Unsupported,
        StorageKind::InvalidInput,
        StorageKind::Serialization,
        StorageKind::Crypto,
        StorageKind::Unavailable,
        StorageKind::Backend,
        StorageKind::Fenced,
        StorageKind::ScopeGenerationRejected,
    ] {
        assert_eq!(serde_json::to_value(marker_kind(kind)).unwrap(), json(kind));
    }
}

#[test]
fn marker_state_decoding_accepts_canonical_state_and_refuses_corruption() {
    let valid = serde_json::json!({
        "format_version": 1,
        "business_revision": "12",
        "remixed": ["reply-a", "reply-b"]
    });
    let markers = Markers::decode(valid).unwrap();
    assert_eq!(markers.revision().unwrap(), 12);
    assert_eq!(markers.remixed, ["reply-a", "reply-b"]);

    for invalid in [
        serde_json::json!({"format_version": 2, "business_revision": "0", "remixed": []}),
        serde_json::json!({"format_version": 1, "business_revision": "01", "remixed": []}),
        serde_json::json!({"format_version": 1, "business_revision": "not-a-number", "remixed": []}),
        serde_json::json!({"format_version": 1, "business_revision": "0", "remixed": ["same", "same"]}),
        serde_json::json!({"format_version": 1, "business_revision": "0", "remixed": [], "profile": "ignored"}),
    ] {
        match Markers::decode(invalid) {
            Err(error) => assert_eq!(error, MarkerFailure::CorruptState),
            Ok(_) => panic!("corrupt marker state was accepted"),
        }
    }
}

#[tokio::test]
async fn linked_module_persists_markers_through_its_authenticated_confidential_peer() {
    let fixture = Fixture::new().await;
    let proxy = fixture.proxy();
    let request = MarkerRequest {
        reply_id: "linked-reply".into(),
    };
    let absent: MarkerOutcome<bool> = proxy
        .call_confidential("WasRemixed", (request.clone(),))
        .await
        .unwrap();
    assert!(!absent.unwrap());

    let marked: MarkerOutcome<MarkRemixedReply> = proxy
        .call_confidential("MarkRemixed", (request.clone(),))
        .await
        .unwrap();
    assert_eq!(
        marked.unwrap(),
        MarkRemixedReply {
            changed: true,
            business_revision: 1,
        }
    );
    let duplicate: MarkerOutcome<MarkRemixedReply> = proxy
        .call_confidential("MarkRemixed", (request,))
        .await
        .unwrap();
    assert_eq!(
        duplicate.unwrap(),
        MarkRemixedReply {
            changed: false,
            business_revision: 1,
        }
    );
    let stored = fixture
        .documents
        .0
        .lock()
        .unwrap()
        .get(MARKER_DOCUMENT_KEY)
        .cloned()
        .unwrap();
    assert_eq!(stored.version.0, 1);
    assert_eq!(stored.doc["remixed"], serde_json::json!(["linked-reply"]));
}

#[tokio::test]
async fn linked_module_refuses_corrupt_marker_state_without_replacing_it() {
    let fixture = Fixture::new().await;
    fixture.documents.0.lock().unwrap().insert(
        MARKER_DOCUMENT_KEY.into(),
        tinystoragedrivers_bus::Versioned {
            id: MARKER_DOCUMENT_KEY.into(),
            version: tinystoragedrivers_bus::Version(1),
            doc: serde_json::json!({"format_version": 2, "business_revision": "0", "remixed": []}),
            meta: None,
        },
    );
    let request = MarkerRequest {
        reply_id: "linked-reply".into(),
    };
    let outcome: MarkerOutcome<MarkRemixedReply> = fixture
        .proxy()
        .call_confidential("MarkRemixed", (request,))
        .await
        .unwrap();
    assert_eq!(outcome, Err(MarkerFailure::CorruptState));
    assert_eq!(
        fixture
            .documents
            .0
            .lock()
            .unwrap()
            .get(MARKER_DOCUMENT_KEY)
            .unwrap()
            .doc["format_version"],
        2
    );
}
