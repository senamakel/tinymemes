use super::*;
use crate::slang::Discovered;
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tinyinference_decisions::{
    Answer, ChoiceAnswer, EvaluationRequest, EvaluationResponse, NoulAnswer, Question, ScoreAnswer,
    Usage,
};

fn outcome(score: u8, max_memes: usize, pick: reading::MemePick) -> Outcome {
    let rating = Rating {
        score,
        tier: if max_memes > 0 {
            Tier::Spicy
        } else {
            Tier::Off
        },
        max_memes,
    };
    Outcome {
        reply: String::new(),
        reading: Some(Reading {
            chat_intent: Intent::Banter,
            reply_intent: Intent::Banter,
            reply_intent_confidence: 1.0,
            frankness: 1.0,
            playful: 1.0,
            serious: 0.0,
            slang_best: None,
            slang_enough: None,
            user_language: None,
            meme: pick,
            reply_matches_user: None,
            meme_p: None,
            meme_weak: None,
        }),
        rating: Some(rating),
        remix: None,
        learned: None,
        skipped: None,
    }
}

#[test]
fn more_memes_requires_allowed_rating_and_no_fitting_catalog_entry() {
    assert!(outcome(8, 1, reading::MemePick::NoneFit).wants_more_memes());
    assert!(!outcome(0, 0, reading::MemePick::NoneFit).wants_more_memes());
    assert!(!outcome(8, 1, reading::MemePick::Unasked).wants_more_memes());
    assert!(!outcome(8, 1, reading::MemePick::Pick("Drake".into())).wants_more_memes());

    let mut missing_reading = outcome(8, 1, reading::MemePick::NoneFit);
    missing_reading.reading = None;
    assert!(!missing_reading.wants_more_memes());
    let mut missing_rating = outcome(8, 1, reading::MemePick::NoneFit);
    missing_rating.rating = None;
    assert!(!missing_rating.wants_more_memes());
}

struct BrokenEvaluator;

#[async_trait]
impl Evaluator for BrokenEvaluator {
    async fn evaluate(
        &self,
        _request: &EvaluationRequest,
    ) -> std::result::Result<EvaluationResponse, BoxError> {
        Err(std::io::Error::other("fixture evaluator unavailable").into())
    }
}

struct UnusedModel;

#[async_trait]
impl ChatModel for UnusedModel {
    async fn complete(&self, _system: &str, _user: &str) -> std::result::Result<String, BoxError> {
        panic!("the rewrite model must not run when reading fails")
    }
}

#[tokio::test]
async fn process_fails_open_to_the_original_when_reading_fails() {
    let engine = MemeEngine::builder(Arc::new(BrokenEvaluator), Arc::new(UnusedModel)).build();
    let original = "Here is the answer.";
    let error = engine.try_process(&[], original).await.unwrap_err();
    assert!(error.to_string().contains("fixture evaluator unavailable"));

    let outcome = engine.process(&[], original).await;
    assert_eq!(outcome.reply, original);
    assert!(outcome.reading.is_none());
    assert!(outcome.rating.is_none());
    assert!(
        outcome
            .skipped
            .unwrap()
            .contains("fixture evaluator unavailable")
    );
}

struct ReadingEvaluator {
    serious: f64,
    reply_matches_user: f64,
    choose_meme: bool,
    slang_enough: f64,
}

#[async_trait]
impl Evaluator for ReadingEvaluator {
    async fn evaluate(
        &self,
        request: &EvaluationRequest,
    ) -> std::result::Result<EvaluationResponse, BoxError> {
        let mut answers = BTreeMap::new();
        for (id, question) in &request.questions {
            let answer = match id.as_str() {
                "chat_intent" | "reply_intent" => Answer::Choice(ChoiceAnswer {
                    choice: "celebration".into(),
                    probabilities: BTreeMap::new(),
                    confidence: 0.9,
                }),
                "frankness" => Answer::Score(ScoreAnswer {
                    score: 4.0,
                    legend: BTreeMap::new(),
                    probabilities: BTreeMap::new(),
                    confidence: 0.9,
                }),
                "user_language" => Answer::Choice(ChoiceAnswer {
                    choice: "hinglish".into(),
                    probabilities: BTreeMap::new(),
                    confidence: 0.9,
                }),
                "meme_best" if self.choose_meme => {
                    let Question::Choice(options) = question else {
                        panic!("meme selection is a choice question")
                    };
                    let title = options
                        .criteria
                        .keys()
                        .find(|title| title.as_str() != "none_fit")
                        .expect("regional meme catalog has candidates")
                        .clone();
                    Answer::Choice(ChoiceAnswer {
                        choice: title,
                        probabilities: BTreeMap::new(),
                        confidence: 0.9,
                    })
                }
                "meme_best" | "slang_best" => Answer::Choice(ChoiceAnswer {
                    choice: "none_fit".into(),
                    probabilities: BTreeMap::new(),
                    confidence: 0.9,
                }),
                "serious" => Answer::Noul(NoulAnswer { noul: self.serious }),
                "reply_matches_user" => Answer::Noul(NoulAnswer {
                    noul: self.reply_matches_user,
                }),
                "slang_enough" => Answer::Noul(NoulAnswer {
                    noul: self.slang_enough,
                }),
                _ => Answer::Noul(NoulAnswer { noul: 0.9 }),
            };
            answers.insert(id.clone(), answer);
        }
        Ok(EvaluationResponse {
            model: "fixture-jev".into(),
            answers,
            usage: Usage::default(),
        })
    }
}

struct FixedModel {
    answer: &'static str,
    calls: AtomicUsize,
}

impl FixedModel {
    fn new(answer: &'static str) -> Arc<Self> {
        Arc::new(Self {
            answer,
            calls: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl ChatModel for FixedModel {
    async fn complete(&self, _system: &str, _user: &str) -> std::result::Result<String, BoxError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(self.answer.to_owned())
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
        _query: &str,
        _intent: Intent,
        _region: &Region,
        _limit: usize,
    ) -> std::result::Result<Vec<Meme>, BoxError> {
        Ok(Vec::new())
    }
}

struct EmptySlangResearcher;

#[async_trait]
impl SlangResearcher for EmptySlangResearcher {
    async fn research(&self, _query: &str) -> std::result::Result<Vec<Discovered>, BoxError> {
        Ok(Vec::new())
    }
}

struct FailingSlangResearcher;

#[async_trait]
impl SlangResearcher for FailingSlangResearcher {
    async fn research(&self, _query: &str) -> std::result::Result<Vec<Discovered>, BoxError> {
        Err(std::io::Error::other("fixture search failed").into())
    }
}

struct PendingSlangResearcher;

#[async_trait]
impl SlangResearcher for PendingSlangResearcher {
    async fn research(&self, _query: &str) -> std::result::Result<Vec<Discovered>, BoxError> {
        std::future::pending().await
    }
}

struct EmptyMemeResearcher;

#[async_trait]
impl MemeResearcher for EmptyMemeResearcher {
    async fn research(&self, _query: &str) -> std::result::Result<Vec<FoundMeme>, BoxError> {
        Ok(Vec::new())
    }
}

fn engine(
    serious: f64,
    reply_matches_user: f64,
    choose_meme: bool,
    model: Arc<FixedModel>,
) -> MemeEngine {
    MemeEngine::builder(
        Arc::new(ReadingEvaluator {
            serious,
            reply_matches_user,
            choose_meme,
            slang_enough: 0.9,
        }),
        model,
    )
    .region(Region::india())
    .policy(RatingPolicy {
        meme_cooldown_turns: 0,
        ..RatingPolicy::default()
    })
    .learn_inline(None)
    .build()
}

#[tokio::test]
async fn engines_rate_straight_chat_and_remix_fun_replies() {
    let model = FixedModel::new("Huge W!");
    let straight = engine(1.0, 0.0, false, model.clone());
    let outcome = straight
        .try_process(&[Turn::user("bad news")], "I am sorry to hear that.")
        .await
        .unwrap();
    assert!(outcome.remix.is_none());
    assert_eq!(outcome.reply, "I am sorry to hear that.");
    assert_eq!(model.calls.load(Ordering::Relaxed), 0);

    let engine = engine(0.0, 0.0, false, model.clone());
    let outcome = engine
        .try_process(&[Turn::user("got the job!")], "Congrats!")
        .await
        .unwrap();
    assert!(outcome.remix.is_some());
    assert_eq!(outcome.reply, "Huge W!");
    assert_eq!(model.calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn engines_only_attach_a_catalog_meme_when_the_register_already_matches() {
    let model = FixedModel::new("unused rewrite");
    let engine = engine(0.0, 0.99, true, model.clone());
    let outcome = engine
        .try_process(&[Turn::user("got the job!")], "Congrats!")
        .await
        .unwrap();
    let remix = outcome.remix.unwrap();
    assert_eq!(remix.mode, RemixMode::MemeOnly);
    assert_eq!(remix.memes.len(), 1);
    assert!(outcome.reply.contains("!["));
    assert_eq!(model.calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn unconfigured_research_hooks_are_noops_and_builder_facades_are_usable() {
    let slang = Arc::new(SlangIndex::default());
    let memes = Arc::new(MemeIndex::default());
    let builder = MemeEngine::builder(
        Arc::new(ReadingEvaluator {
            serious: 0.0,
            reply_matches_user: 0.0,
            choose_meme: false,
            slang_enough: 0.9,
        }),
        FixedModel::new("rewrite"),
    )
    .source(Arc::new(EmptySource))
    .policy(RatingPolicy::default())
    .window(Window::default())
    .region(Region::india())
    .slang_index(slang.clone())
    .learn_inline(None)
    .meme_index(memes.clone())
    .openjev(true);
    assert_eq!(format!("{builder:?}"), "MemeEngineBuilder { .. }");
    let engine = builder.build();
    assert!(format!("{engine:?}").contains("MemeEngine {"));
    assert!(Arc::ptr_eq(engine.slang_index(), &slang));
    assert!(Arc::ptr_eq(engine.meme_index(), &memes));
    assert!(engine.learn_slang(Intent::Banter).await.unwrap().is_none());
    assert!(
        engine
            .learn_for_reply(Intent::Banter, "good news")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        engine
            .learn_memes(Intent::Banter, "good news")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn configured_research_hooks_return_empty_reports_without_external_io() {
    let engine = MemeEngine::builder(
        Arc::new(ReadingEvaluator {
            serious: 0.0,
            reply_matches_user: 0.0,
            choose_meme: false,
            slang_enough: 0.9,
        }),
        FixedModel::new(r#"{"concept":"work celebration"}"#),
    )
    .region(Region::india())
    .researcher(Arc::new(EmptySlangResearcher))
    .meme_researcher(Arc::new(EmptyMemeResearcher))
    .learn_inline(Some(Duration::from_secs(1)))
    .build();
    assert!(engine.learn_slang(Intent::Banter).await.unwrap().is_some());
    assert!(
        engine
            .learn_for_reply(Intent::Banter, "got the job")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        engine
            .learn_memes(Intent::Banter, "got the job")
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn inline_slang_research_failures_and_zero_budget_timeouts_fail_open_to_rewrite() {
    for (researcher, budget) in [
        (
            Arc::new(FailingSlangResearcher) as Arc<dyn SlangResearcher>,
            Duration::from_secs(1),
        ),
        (
            Arc::new(PendingSlangResearcher) as Arc<dyn SlangResearcher>,
            Duration::ZERO,
        ),
    ] {
        let engine = MemeEngine::builder(
            Arc::new(ReadingEvaluator {
                serious: 0.0,
                reply_matches_user: 0.0,
                choose_meme: false,
                slang_enough: 0.0,
            }),
            FixedModel::new("Huge W!"),
        )
        .region(Region::india())
        .researcher(researcher)
        .learn_inline(Some(budget))
        .build();
        let outcome = engine
            .try_process(&[Turn::user("got the job")], "Congrats!")
            .await
            .unwrap();
        assert!(outcome.remix.is_some());
        assert_eq!(outcome.reply, "Huge W!");
        assert!(outcome.learned.is_none());
    }
}

#[tokio::test]
async fn a_recent_assistant_meme_uses_the_rating_cooldown() {
    let engine = MemeEngine::builder(
        Arc::new(ReadingEvaluator {
            serious: 0.0,
            reply_matches_user: 0.0,
            choose_meme: false,
            slang_enough: 0.9,
        }),
        FixedModel::new("Huge W!"),
    )
    .region(Region::india())
    .policy(RatingPolicy {
        meme_cooldown_turns: 1,
        ..RatingPolicy::default()
    })
    .learn_inline(None)
    .build();
    let conversation = [
        Turn::user("old turn"),
        Turn::assistant("![Old meme](https://media.example/old.webp)"),
        Turn::user("got the job"),
    ];
    let outcome = engine
        .try_process(&conversation, "Congrats!")
        .await
        .unwrap();
    assert_eq!(outcome.rating.unwrap().max_memes, 0);
    assert!(outcome.remix.is_some());
}

#[test]
fn standalone_constructors_require_and_apply_local_credentials() {
    assert!(
        MemeEngine::from_standalone_config(EnvConfig::default(), None)
            .unwrap_err()
            .to_string()
            .contains("set TINYMEMES_OPENROUTER_KEY")
    );

    let fallback =
        MemeEngine::from_standalone_config(EnvConfig::default(), Some("fallback-key".into()))
            .unwrap()
            .build();
    assert!(format!("{fallback:?}").contains("MemeEngine {"));

    let configured = EnvConfig {
        openrouter_key: Some("unit-test-key".into()),
        model: Some("fixture-model".into()),
        jev: Some(JevMode::Llm),
        ..EnvConfig::default()
    };
    let engine = MemeEngine::from_standalone_config(configured, Some("ignored-fallback".into()))
        .unwrap()
        .build();
    assert!(format!("{engine:?}").contains("MemeEngine {"));
    let openrouter = MemeEngine::openrouter("unit-test-key", "fixture-model")
        .unwrap()
        .build();
    assert!(format!("{openrouter:?}").contains("MemeEngine {"));

    let missing_route_key = EnvConfig {
        openrouter_key: Some("unit-test-key".into()),
        jev: Some(JevMode::TypeSafe),
        ..EnvConfig::default()
    };
    assert!(MemeEngine::from_standalone_config(missing_route_key, None).is_err());
}
