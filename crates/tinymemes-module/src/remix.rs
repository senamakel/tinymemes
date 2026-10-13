//! The remix operation: engine adapters over [`Host`], timeout, drain, service.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tinymemes::decisions::{EvaluationRequest, EvaluationResponse};
use tinymemes::{
    BoxError, ChatModel, Evaluator, IndexPolicy, MemeEngine, MemeIndex, MemeIndexPolicy, Region,
    Role, SearchHit, SearchResearcher, SlangIndex, Turn, WebSearch,
};
use tinymemes_bus::{
    CompleteRequest, DeliverRequest, IndexKind, REMIX_INTERFACE, RemixRefusal, RemixRequest,
    RemixResponse, RemixResult, SearchRequest, SkipReason, WireRole, remix_methods,
};
use tokio::sync::Notify;

use crate::host::{BusHost, SharedHost};

struct HostModel(SharedHost);
#[async_trait]
impl ChatModel for HostModel {
    async fn complete(&self, system: &str, user: &str) -> Result<String, BoxError> {
        let request = CompleteRequest {
            system: system.into(),
            user: user.into(),
        };
        self.0
            .complete(request)
            .await
            .map_err(|e| format!("host complete failed: {e:?}").into())
    }
}

struct HostEvaluator(SharedHost);
#[async_trait]
impl Evaluator for HostEvaluator {
    async fn evaluate(&self, request: &EvaluationRequest) -> Result<EvaluationResponse, BoxError> {
        let body = serde_json::to_value(request)?;
        let reply = self
            .0
            .evaluate(body)
            .await
            .map_err(|e| format!("host evaluate failed: {e:?}"))?;
        Ok(serde_json::from_value(reply)?)
    }
}

struct HostSearch(SharedHost);
#[async_trait]
impl WebSearch for HostSearch {
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SearchHit>, BoxError> {
        let request = SearchRequest {
            query: query.into(),
            max_results: u32::try_from(max_results).unwrap_or(u32::MAX),
        };
        let hits = self
            .0
            .search(request)
            .await
            .map_err(|e| format!("host search failed: {e:?}"))?;
        Ok(hits
            .into_iter()
            .map(|h| SearchHit {
                title: h.title,
                url: h.url,
                snippet: h.snippet,
            })
            .collect())
    }
}

fn turns(request: &RemixRequest) -> Vec<Turn> {
    request
        .conversation
        .iter()
        .map(|t| Turn {
            role: match t.role {
                WireRole::User => Role::User,
                WireRole::Assistant => Role::Assistant,
            },
            text: t.text.clone(),
            remixed: t.remixed,
        })
        .collect()
}

fn unchanged(reply: &str, reason: SkipReason) -> RemixResponse {
    RemixResponse {
        reply: reply.to_owned(),
        remixed: false,
        skipped: Some(reason),
        score: None,
        wants_more_memes: false,
    }
}

async fn load<T>(
    host: &SharedHost,
    kind: IndexKind,
    parse: impl Fn(&str) -> Option<T>,
    fresh: impl Fn() -> T,
) -> T {
    match host.load_index(kind).await {
        Ok(Some(json)) => parse(&json).unwrap_or_else(&fresh),
        _ => fresh(),
    }
}

/// Run one remix against `host`. Never errors: every failure returns the
/// original reply with a content-free reason. Dropping the returned future
/// cancels any in-flight host callback.
pub(crate) async fn run(host: SharedHost, request: RemixRequest) -> RemixResponse {
    if !request.enabled {
        return unchanged(&request.reply, SkipReason::Disabled);
    }
    let budget = Duration::from_millis(request.timeout_ms);
    let work = remix(host.clone(), &request);
    let response = match tokio::time::timeout(budget, work).await {
        Ok(response) => response,
        Err(_) => unchanged(&request.reply, SkipReason::Timeout),
    };
    if request.deliver {
        let delivery = DeliverRequest {
            reply: response.reply.clone(),
            remixed: response.remixed,
        };
        // Delivery is the host's; its failure cannot change the reply.
        let _ = tokio::time::timeout(budget, host.deliver(delivery)).await;
    }
    response
}

async fn remix(host: SharedHost, request: &RemixRequest) -> RemixResponse {
    let slang = Arc::new(
        load(
            &host,
            IndexKind::Slang,
            |j| SlangIndex::from_json(j, IndexPolicy::default()).ok(),
            || SlangIndex::new(IndexPolicy::default()),
        )
        .await,
    );
    let memes = Arc::new(
        load(
            &host,
            IndexKind::Memes,
            |j| MemeIndex::from_json(j, MemeIndexPolicy::default()).ok(),
            || MemeIndex::new(MemeIndexPolicy::default()),
        )
        .await,
    );
    let model: Arc<dyn ChatModel> = Arc::new(HostModel(host.clone()));
    let region = match request.region.as_deref() {
        Some(code) if code.eq_ignore_ascii_case("global") => Region::global(),
        _ => Region::default(),
    };
    let engine = MemeEngine::builder(Arc::new(HostEvaluator(host.clone())), model.clone())
        .region(region)
        .slang_index(slang.clone())
        .meme_index(memes.clone())
        .researcher(Arc::new(SearchResearcher::new(
            Arc::new(HostSearch(host.clone())),
            model,
        )))
        .source(Arc::new(tinymemes::source::Imgflip::new(reqwest_client())))
        .build();
    let outcome = engine.process(&turns(request), &request.reply).await;
    let remixed = outcome.remix.is_some() && outcome.reply != request.reply;
    if remixed || outcome.learned.is_some() {
        // Best effort: a lost save only loses usage counts.
        let _ = host.save_index(IndexKind::Slang, slang.to_json()).await;
        let _ = host.save_index(IndexKind::Memes, memes.to_json()).await;
    }
    let skipped = match (&outcome.skipped, remixed) {
        (_, true) => None,
        (Some(_), _) if outcome.rating.is_none() => Some(SkipReason::Failed),
        _ => Some(SkipReason::BelowThreshold),
    };
    RemixResponse {
        wants_more_memes: outcome.wants_more_memes(),
        score: outcome.rating.map(|r| r.score),
        reply: outcome.reply,
        remixed,
        skipped,
    }
}

fn reqwest_client() -> reqwest::Client {
    reqwest::Client::new()
}

/// Counts in-flight remixes so a close can wait for them and refuse new work.
#[derive(Default)]
pub(crate) struct Drain {
    closing: AtomicBool,
    active: AtomicUsize,
    idle: Notify,
}

/// Released on drop, including when the call future is cancelled.
pub(crate) struct Permit(Arc<Drain>);
impl Drop for Permit {
    fn drop(&mut self) {
        if self.0.active.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.0.idle.notify_waiters();
        }
    }
}

impl Drain {
    /// Admit one request, or refuse it when closing.
    pub(crate) fn admit(self: &Arc<Self>) -> Option<Permit> {
        self.active.fetch_add(1, Ordering::SeqCst);
        if self.closing.load(Ordering::SeqCst) {
            drop(Permit(self.clone()));
            return None;
        }
        Some(Permit(self.clone()))
    }
    /// Refuse new work without waiting.
    pub(crate) fn begin_close(&self) {
        self.closing.store(true, Ordering::SeqCst);
    }
    /// Refuse new work and wait until every admitted request has finished.
    /// The module ABI has no async close hook, so the service only calls
    /// [`Self::begin_close`] when it is dropped; this is the draining half.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn close(&self) {
        self.begin_close();
        loop {
            let notified = self.idle.notified();
            if self.active.load(Ordering::SeqCst) == 0 {
                return;
            }
            notified.await;
        }
    }
}

pub(crate) struct RemixService {
    pub(crate) connection: tinybus::Connection,
    pub(crate) drain: Arc<Drain>,
}

impl Drop for RemixService {
    fn drop(&mut self) {
        self.drain.begin_close();
    }
}

fn encode(result: RemixResult) -> tinybus::Result<serde_json::Value> {
    serde_json::to_value(result).map_err(|_| tinybus::Error::failed("invalid remix response"))
}

#[async_trait]
impl tinybus::Interface for RemixService {
    fn name(&self) -> tinybus::InterfaceName {
        REMIX_INTERFACE.try_into().unwrap()
    }
    fn members(&self) -> Vec<tinybus::MemberName> {
        tinymemes_bus::REMIX_METHODS
            .iter()
            .map(|m| (*m).try_into().unwrap())
            .collect()
    }
    async fn call(
        &self,
        _: &tinybus::MemberName,
        _: serde_json::Value,
    ) -> tinybus::Result<serde_json::Value> {
        encode(Err(RemixRefusal::Unauthenticated))
    }
    async fn call_with_context(
        &self,
        member: &tinybus::MemberName,
        args: serde_json::Value,
        context: &tinybus::CallContext,
    ) -> tinybus::Result<serde_json::Value> {
        if member.as_str() != remix_methods::REMIX {
            return Err(tinybus::Error::failed("unknown remix operation"));
        }
        let (request,): (RemixRequest,) = serde_json::from_value(args)
            .map_err(|_| tinybus::Error::failed("invalid remix request"))?;
        let Some(caller) = context.authenticated_sender().cloned() else {
            return encode(Err(RemixRefusal::Unauthenticated));
        };
        let Some(_permit) = self.drain.admit() else {
            return encode(Err(RemixRefusal::Closing));
        };
        let proxy = self.connection.proxy(
            caller.as_str(),
            tinymemes_bus::HOST_PATH,
            tinymemes_bus::HOST_INTERFACE,
        )?;
        let host: SharedHost = Arc::new(BusHost(proxy));
        encode(Ok(run(host, request).await))
    }
}

#[cfg(test)]
#[path = "remix_tests.rs"]
mod tests;
