use super::*;
use crate::host::Host;
use std::sync::Mutex;
use tinymemes::Turn;
use tinymemes::decisions::{Answer, ChoiceAnswer, Usage};
use tinymemes_bus::{DeliverRequest, HostResult, IndexKind, SearchResult};

/// A host whose `Evaluate` and `Complete` behaviour is scripted.
struct Scripted {
    evaluate: HostResult<serde_json::Value>,
    calls: Mutex<Vec<&'static str>>,
    search_result: HostResult<Vec<SearchResult>>,
    search_request: Mutex<Option<SearchRequest>>,
}

impl Scripted {
    fn new(evaluate: HostResult<serde_json::Value>) -> Arc<Self> {
        Arc::new(Self {
            evaluate,
            calls: Mutex::new(Vec::new()),
            search_result: Ok(vec![SearchResult {
                title: "guide".into(),
                url: "https://example.test/guide".into(),
                snippet: Some("snippet".into()),
            }]),
            search_request: Mutex::new(None),
        })
    }
}

#[async_trait]
impl Host for Scripted {
    async fn complete(&self, _: CompleteRequest) -> HostResult<String> {
        self.calls.lock().unwrap().push("complete");
        Ok(r#"{"chat_intent": {"choice": "celebration"}, "reply_intent": {"choice": "celebration"},
            "frankness": {"level": 3}, "playful": {"p": 0.8}, "serious": {"p": 0.05}}"#
            .into())
    }
    async fn evaluate(&self, _: serde_json::Value) -> HostResult<serde_json::Value> {
        self.calls.lock().unwrap().push("evaluate");
        self.evaluate.clone()
    }
    async fn search(&self, request: SearchRequest) -> HostResult<Vec<SearchResult>> {
        self.calls.lock().unwrap().push("search");
        *self.search_request.lock().unwrap() = Some(request);
        self.search_result.clone()
    }
    async fn load_index(&self, _: IndexKind) -> HostResult<Option<String>> {
        Ok(None)
    }
    async fn save_index(&self, _: IndexKind, _: String) -> HostResult<()> {
        Ok(())
    }
    async fn deliver(&self, _: DeliverRequest) -> HostResult<()> {
        Ok(())
    }
    async fn end(&self) -> HostResult<()> {
        Ok(())
    }
}

struct EmptySource;

#[async_trait]
impl MemeSource for EmptySource {
    fn name(&self) -> &'static str {
        "empty"
    }

    async fn search(
        &self,
        _: &str,
        _: tinymemes::Intent,
        _: &tinymemes::Region,
        _: usize,
    ) -> std::result::Result<Vec<tinymemes::Meme>, BoxError> {
        Ok(Vec::new())
    }
}

fn question_request() -> EvaluationRequest {
    tinymemes::reading::reading_request(
        &[Turn::user("got the job!!")],
        "Congrats!",
        "India",
        &[],
        &[],
        tinymemes::Window::default(),
        false,
    )
}

fn evaluator(host: &Arc<Scripted>) -> HostEvaluator {
    let shared: SharedHost = host.clone();
    let model: Arc<dyn ChatModel> = Arc::new(HostModel(shared.clone()));
    HostEvaluator::new(shared, model)
}

#[tokio::test]
async fn a_host_without_jev_is_answered_by_the_chat_model() {
    let host = Scripted::new(Err(HostFailure::Unsupported));
    let jev = evaluator(&host);
    let response = jev.evaluate(&question_request()).await.unwrap();
    assert_eq!(response.model, "llm-fallback");
    assert!(
        matches!(response.answers["chat_intent"], Answer::Choice(ChoiceAnswer { ref choice, .. }) if choice == "celebration")
    );
    // The host is asked once; afterwards the fallback is used directly.
    jev.evaluate(&question_request()).await.unwrap();
    let calls = host.calls.lock().unwrap().clone();
    assert_eq!(calls.iter().filter(|c| **c == "evaluate").count(), 1);
    assert_eq!(calls.iter().filter(|c| **c == "complete").count(), 2);
}

#[tokio::test]
async fn a_failing_host_jev_is_not_papered_over_by_the_chat_model() {
    let host = Scripted::new(Err(HostFailure::Backend));
    let error = evaluator(&host)
        .evaluate(&question_request())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("host evaluate failed"));
    assert!(!host.calls.lock().unwrap().contains(&"complete"));
}

#[tokio::test]
async fn the_host_jev_answer_is_used_when_it_has_one() {
    let answer = EvaluationResponse {
        model: "jev-latest".into(),
        answers: [(
            "chat_intent".to_owned(),
            Answer::Choice(ChoiceAnswer {
                choice: "celebration".into(),
                probabilities: Default::default(),
                confidence: 0.9,
            }),
        )]
        .into(),
        usage: Usage::default(),
    };
    let host = Scripted::new(Ok(serde_json::to_value(&answer).unwrap()));
    let response = evaluator(&host)
        .evaluate(&question_request())
        .await
        .unwrap();
    assert_eq!(response.model, "jev-latest");
    assert!(!host.calls.lock().unwrap().contains(&"complete"));
}

#[tokio::test]
async fn host_search_saturates_limits_and_maps_wire_hits() {
    let host = Scripted::new(Err(HostFailure::Unsupported));
    let shared: SharedHost = host.clone();
    let hits = HostSearch(shared).search("term", usize::MAX).await.unwrap();
    assert_eq!(
        hits,
        [SearchHit {
            title: "guide".into(),
            url: "https://example.test/guide".into(),
            snippet: Some("snippet".into()),
        }]
    );
    assert_eq!(
        host.search_request
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .max_results,
        u32::MAX
    );
}

#[test]
fn engine_build_uses_host_fallback_and_env_overrides_without_network_calls() {
    let request = |research, cooldown| RemixRequest {
        conversation: Vec::new(),
        reply: "reply".into(),
        region: Some("global".into()),
        enabled: true,
        timeout_ms: 20_000,
        deliver: false,
        correlation: "build-test".into(),
        reply_id: None,
        options: tinymemes_bus::RemixOptions {
            meme_cooldown_turns: cooldown,
            research,
        },
    };
    let host = Scripted::new(Err(HostFailure::Unsupported));
    let shared: SharedHost = host.clone();
    let build = |env: EnvConfig, request: RemixRequest| {
        let deps = Deps {
            env,
            source: Arc::new(EmptySource),
        };
        let _built = super::build(
            &shared,
            &request,
            &deps,
            SlangIndex::default(),
            MemeIndex::default(),
        );
    };

    build(EnvConfig::default(), request(false, Some(0)));
    build(
        EnvConfig::from_lookup(|name| match name {
            "TINYMEMES_JEV" => Some("typesafe".into()),
            _ => None,
        }),
        request(true, None),
    );
    build(
        EnvConfig::from_lookup(|name| match name {
            "TINYMEMES_JEV" => Some("openrouter".into()),
            "TINYMEMES_OPENROUTER_KEY" => Some("test-only-key".into()),
            "TINYMEMES_MODEL" => Some("fixture-model".into()),
            _ => None,
        }),
        request(true, Some(1)),
    );
    build(
        EnvConfig::from_lookup(|name| match name {
            "TINYMEMES_JEV" => Some("llm".into()),
            "TINYMEMES_OPENROUTER_KEY" => Some("test-only-key".into()),
            _ => None,
        }),
        request(false, None),
    );
    assert!(host.calls.lock().unwrap().is_empty());
}
