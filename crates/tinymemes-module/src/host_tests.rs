use super::*;
use std::sync::Mutex;
use tinybus::{
    Connection, Interface, MemberName, ObjectPath, broker::Broker, transport::memory::MemoryBus,
};
use tinymemes_bus::{
    CompleteRequest, DeliverRequest, EndRequest, EvaluateEnvelope, HOST_INTERFACE, HOST_METHODS,
    HOST_PATH, HostFailure, IndexKind, LoadIndexRequest, SaveIndexRequest, SearchRequest,
    SearchResult,
};

struct CallRecorder {
    calls: Mutex<Vec<(String, serde_json::Value)>>,
    malformed_reply: bool,
}

#[async_trait::async_trait]
impl Interface for CallRecorder {
    fn name(&self) -> tinybus::InterfaceName {
        HOST_INTERFACE.try_into().unwrap()
    }

    fn members(&self) -> Vec<MemberName> {
        HOST_METHODS
            .iter()
            .map(|member| MemberName::new(*member).unwrap())
            .collect()
    }

    async fn call(
        &self,
        member: &MemberName,
        args: serde_json::Value,
    ) -> tinybus::Result<serde_json::Value> {
        self.calls
            .lock()
            .unwrap()
            .push((member.as_str().to_owned(), args.clone()));
        if self.malformed_reply {
            return Ok(serde_json::Value::Null);
        }
        let reply = match member.as_str() {
            "Complete" => serde_json::to_value(Ok::<_, HostFailure>("answer".to_owned())),
            "Evaluate" => {
                let (request,): (EvaluateEnvelope,) = serde_json::from_value(args).unwrap();
                serde_json::to_value(Ok::<_, HostFailure>(EvaluateEnvelope {
                    correlation: String::new(),
                    body: request.body,
                }))
            }
            "Search" => serde_json::to_value(Ok::<_, HostFailure>(vec![SearchResult {
                title: "result".into(),
                url: "https://search.example/result".into(),
                snippet: Some("snippet".into()),
            }])),
            "LoadIndex" => {
                serde_json::to_value(Ok::<_, HostFailure>(Some::<String>("snapshot".into())))
            }
            "SaveIndex" | "Deliver" | "End" => serde_json::to_value(Ok::<_, HostFailure>(())),
            other => panic!("unexpected host method {other}"),
        };
        Ok(reply.unwrap())
    }
}

struct Fixture {
    host: BusHost,
    recorder: Arc<CallRecorder>,
    _server: Connection,
    _task: tokio::task::JoinHandle<tinybus::Result<()>>,
}

impl Fixture {
    async fn new(malformed_reply: bool) -> Self {
        let bus = MemoryBus::new();
        let broker = Broker::new();
        let task = broker.spawn(bus.clone());
        let server = Connection::connect(bus.connect().await.unwrap())
            .await
            .unwrap();
        let recorder = Arc::new(CallRecorder {
            calls: Mutex::new(Vec::new()),
            malformed_reply,
        });
        server
            .serve_at(ObjectPath::new(HOST_PATH).unwrap(), recorder.clone())
            .await
            .unwrap();
        let destination = server.unique_name().unwrap().as_str().to_owned();
        let caller = Connection::connect(bus.connect().await.unwrap())
            .await
            .unwrap();
        let proxy = caller
            .proxy(destination, HOST_PATH, HOST_INTERFACE)
            .unwrap();
        Self {
            host: BusHost::new(proxy, "turn-42".into()),
            recorder,
            _server: server,
            _task: task,
        }
    }

    fn last_call(&self) -> (String, serde_json::Value) {
        self.recorder.calls.lock().unwrap().last().unwrap().clone()
    }
}

#[tokio::test]
async fn bus_host_forwards_all_callback_types_with_the_request_correlation() {
    let fixture = Fixture::new(false).await;
    let complete = fixture
        .host
        .complete(CompleteRequest {
            correlation: "caller-value".into(),
            system: "system".into(),
            user: "user".into(),
        })
        .await
        .unwrap();
    assert_eq!(complete, "answer");
    let (member, args) = fixture.last_call();
    assert_eq!(member, "Complete");
    assert_eq!(args[0]["correlation"], "turn-42");

    let body = serde_json::json!({"question": "meaning"});
    assert_eq!(fixture.host.evaluate(body.clone()).await.unwrap(), body);
    let (member, args) = fixture.last_call();
    assert_eq!(member, "Evaluate");
    assert_eq!(args[0]["correlation"], "turn-42");
    assert_eq!(args[0]["body"], body);

    let hits = fixture
        .host
        .search(SearchRequest {
            correlation: String::new(),
            query: "query".into(),
            max_results: u32::MAX,
        })
        .await
        .unwrap();
    assert_eq!(hits[0].title, "result");
    let (member, args) = fixture.last_call();
    assert_eq!(member, "Search");
    assert_eq!(args[0]["correlation"], "turn-42");
    assert_eq!(args[0]["max_results"], u32::MAX);

    assert_eq!(
        fixture.host.load_index(IndexKind::Memes).await.unwrap(),
        Some("snapshot".into())
    );
    let (member, args) = fixture.last_call();
    assert_eq!(member, "LoadIndex");
    assert_eq!(args[0]["correlation"], "turn-42");

    fixture
        .host
        .save_index(IndexKind::Slang, "index-json".into())
        .await
        .unwrap();
    let (member, args) = fixture.last_call();
    assert_eq!(member, "SaveIndex");
    assert_eq!(args[0]["correlation"], "turn-42");
    assert_eq!(args[0]["json"], "index-json");

    fixture
        .host
        .deliver(DeliverRequest {
            correlation: String::new(),
            reply: "reply".into(),
            remixed: true,
        })
        .await
        .unwrap();
    let (member, args) = fixture.last_call();
    assert_eq!(member, "Deliver");
    assert_eq!(args[0]["correlation"], "turn-42");

    fixture.host.end().await.unwrap();
    let (member, args) = fixture.last_call();
    assert_eq!(member, "End");
    assert_eq!(args[0]["correlation"], "turn-42");

    let end: EndRequest = serde_json::from_value(args[0].clone()).unwrap();
    assert_eq!(end.correlation, "turn-42");
    let _: (CompleteRequest,) =
        serde_json::from_value(fixture.recorder.calls.lock().unwrap()[0].1.clone()).unwrap();
    let _: (LoadIndexRequest,) =
        serde_json::from_value(fixture.recorder.calls.lock().unwrap()[3].1.clone()).unwrap();
    let _: (SaveIndexRequest,) =
        serde_json::from_value(fixture.recorder.calls.lock().unwrap()[4].1.clone()).unwrap();
}

#[tokio::test]
async fn bus_host_sanitizes_unreadable_callback_replies_as_backend_failures() {
    let fixture = Fixture::new(true).await;
    let error = fixture
        .host
        .complete(CompleteRequest {
            correlation: String::new(),
            system: String::new(),
            user: String::new(),
        })
        .await
        .unwrap_err();
    assert_eq!(error, HostFailure::Backend);
}
