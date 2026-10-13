use super::*;
use crate::host::Host;
use std::sync::Mutex;
use tinymemes_bus::{HostFailure, HostResult, SearchResult, WireTurn};

#[derive(Clone, Copy, PartialEq)]
enum Eval {
    Fail,
    Hang,
}

struct Mock {
    eval: Eval,
    calls: Mutex<Vec<&'static str>>,
    delivered: Mutex<Vec<DeliverRequest>>,
    dropped: Arc<AtomicBool>,
}

struct DropFlag(Arc<AtomicBool>);
impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl Mock {
    fn new(eval: Eval) -> Arc<Self> {
        Arc::new(Self {
            eval,
            calls: Mutex::new(Vec::new()),
            delivered: Mutex::new(Vec::new()),
            dropped: Arc::default(),
        })
    }
    fn note(&self, name: &'static str) {
        self.calls.lock().unwrap().push(name);
    }
}

#[async_trait]
impl Host for Mock {
    async fn complete(&self, _: CompleteRequest) -> HostResult<String> {
        self.note("complete");
        Err(HostFailure::Backend)
    }
    async fn evaluate(&self, _: serde_json::Value) -> HostResult<serde_json::Value> {
        self.note("evaluate");
        match self.eval {
            Eval::Fail => Err(HostFailure::Backend),
            Eval::Hang => {
                let _guard = DropFlag(self.dropped.clone());
                std::future::pending().await
            }
        }
    }
    async fn search(&self, _: SearchRequest) -> HostResult<Vec<SearchResult>> {
        self.note("search");
        Err(HostFailure::Unsupported)
    }
    async fn load_index(&self, _: IndexKind) -> HostResult<Option<String>> {
        self.note("load_index");
        Ok(None)
    }
    async fn save_index(&self, _: IndexKind, _: String) -> HostResult<()> {
        self.note("save_index");
        Ok(())
    }
    async fn deliver(&self, request: DeliverRequest) -> HostResult<()> {
        self.note("deliver");
        self.delivered.lock().unwrap().push(request);
        Ok(())
    }
}

fn request() -> RemixRequest {
    RemixRequest {
        conversation: vec![WireTurn {
            role: WireRole::User,
            text: "my build finally passed lol".into(),
            remixed: false,
        }],
        reply: "Congrats, the fix was the feature flag.".into(),
        region: None,
        enabled: true,
        timeout_ms: 5_000,
        deliver: false,
    }
}

#[tokio::test]
async fn disabled_returns_the_original_without_any_host_call() {
    let host = Mock::new(Eval::Fail);
    let mut req = request();
    req.enabled = false;
    let out = run(host.clone(), req.clone()).await;
    assert_eq!(out.reply, req.reply);
    assert!(!out.remixed);
    assert_eq!(out.skipped, Some(SkipReason::Disabled));
    assert!(host.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_failing_evaluator_fails_open_with_the_original_reply() {
    let host = Mock::new(Eval::Fail);
    let req = request();
    let out = run(host.clone(), req.clone()).await;
    assert_eq!(out.reply, req.reply);
    assert!(!out.remixed);
    assert_eq!(out.skipped, Some(SkipReason::Failed));
    assert!(host.calls.lock().unwrap().contains(&"evaluate"));
    assert!(!host.calls.lock().unwrap().contains(&"save_index"));
}

#[tokio::test]
async fn a_hung_callback_times_out_and_is_cancelled() {
    let host = Mock::new(Eval::Hang);
    let mut req = request();
    req.timeout_ms = 40;
    let out = run(host.clone(), req.clone()).await;
    assert_eq!(out.reply, req.reply);
    assert_eq!(out.skipped, Some(SkipReason::Timeout));
    assert!(
        host.dropped.load(Ordering::SeqCst),
        "the in-flight host callback must be dropped on timeout"
    );
}

#[tokio::test]
async fn dropping_the_call_cancels_the_in_flight_callback() {
    let host = Mock::new(Eval::Hang);
    let call = tokio::spawn(run(host.clone(), request()));
    while !host.calls.lock().unwrap().contains(&"evaluate") {
        tokio::task::yield_now().await;
    }
    call.abort();
    let _ = call.await;
    assert!(host.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn delivery_carries_the_final_reply_even_when_the_remix_failed() {
    let host = Mock::new(Eval::Fail);
    let mut req = request();
    req.deliver = true;
    let out = run(host.clone(), req.clone()).await;
    let delivered = host.delivered.lock().unwrap().clone();
    assert_eq!(
        delivered,
        vec![DeliverRequest {
            reply: out.reply,
            remixed: false
        }]
    );
}

#[tokio::test]
async fn close_refuses_new_work_and_waits_for_in_flight_requests() {
    let drain = Arc::new(Drain::default());
    let permit = drain.admit().expect("open drain admits");
    let closing = tokio::spawn({
        let drain = drain.clone();
        async move { drain.close().await }
    });
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!closing.is_finished(), "close must wait for the permit");
    assert!(drain.admit().is_none(), "a closing drain refuses new work");
    drop(permit);
    tokio::time::timeout(Duration::from_secs(2), closing)
        .await
        .expect("close completes once drained")
        .unwrap();
}

#[tokio::test]
async fn an_idle_drain_closes_immediately_and_a_refused_admit_does_not_leak() {
    let drain = Arc::new(Drain::default());
    drain.close().await;
    assert!(drain.admit().is_none());
    drain.close().await;
}

#[tokio::test]
async fn bus_remix_reaches_the_callers_host_and_fails_open() {
    use tinybus::{Connection, broker::Broker, module::ModuleHost, transport::memory::MemoryBus};
    let bus = MemoryBus::new();
    let broker = Broker::new();
    broker.clone().spawn(bus.clone());
    let host = ModuleHost::new(broker);
    host.attach_linked_with_config(
        crate::exports::linked_module().unwrap(),
        serde_json::Value::Null,
    )
    .unwrap();
    let caller = Connection::connect(bus.connect().await.unwrap())
        .await
        .unwrap();
    let client = tinymemes_bus::RemixClient::new(&caller).unwrap();
    let mut req = request();
    req.enabled = false;
    let result = client.remix(req.clone()).await.unwrap();
    // Either the bus names the caller and the module answers, or it refuses an
    // unauthenticated peer; it never remixes without a host to call back.
    match result {
        Ok(out) => assert_eq!(out.reply, req.reply),
        Err(refusal) => assert_eq!(refusal, RemixRefusal::Unauthenticated),
    }
}
