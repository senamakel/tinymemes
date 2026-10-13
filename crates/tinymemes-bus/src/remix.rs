//! The remix contract: one request/response member the host calls, and the
//! host-callback vocabulary the module calls back into while it works.
//!
//! The module owns the algorithm (reading, rating, rewriting, indexes). The
//! host owns inference, evaluation, web search, scoped storage and delivery,
//! and serves them at [`HOST_INTERFACE`]. Everything here is plain data.
use serde::{Deserialize, Serialize};

/// Remix service interface the module claims.
pub const REMIX_INTERFACE: &str = "ai.tinyhumans.tinymemes.Remix";
/// Remix service object path.
pub const REMIX_PATH: &str = "/ai/tinyhumans/tinymemes/Remix";
/// Interface a host serves back to the module (the `MlHost` direction).
pub const HOST_INTERFACE: &str = "ai.tinyhumans.tinymemes.Host";
/// Object path a host serves [`HOST_INTERFACE`] at.
pub const HOST_PATH: &str = "/ai/tinyhumans/tinymemes/Host";

/// The contract version. Bump the minor for an additive member or field and
/// the major for anything a bound peer could misread.
pub const CONTRACT_VERSION: (u32, u32) = (1, 0);

/// Whether a host built against [`CONTRACT_VERSION`] can bind to `module`.
#[must_use]
pub fn is_compatible(module: (u32, u32)) -> bool {
    module.0 == CONTRACT_VERSION.0 && module.1 >= CONTRACT_VERSION.1
}

/// One constant per member of [`REMIX_INTERFACE`].
pub mod remix_methods {
    /// Read, rate and remix one reply.
    pub const REMIX: &str = "Remix";
}

/// One constant per member of [`HOST_INTERFACE`].
pub mod host_methods {
    /// One system + user chat completion (`ChatModel`).
    pub const COMPLETE: &str = "Complete";
    /// One Jev evaluation (`Evaluator`).
    pub const EVALUATE: &str = "Evaluate";
    /// One web search (`WebSearch`).
    pub const SEARCH: &str = "Search";
    /// Read one scoped index document.
    pub const LOAD_INDEX: &str = "LoadIndex";
    /// Write one scoped index document.
    pub const SAVE_INDEX: &str = "SaveIndex";
    /// Hand the final reply to the host for delivery.
    pub const DELIVER: &str = "Deliver";
}

/// Every member of [`REMIX_INTERFACE`].
pub const REMIX_METHODS: &[&str] = &[remix_methods::REMIX];
/// Every member of [`HOST_INTERFACE`].
pub const HOST_METHODS: &[&str] = &[
    host_methods::COMPLETE,
    host_methods::EVALUATE,
    host_methods::SEARCH,
    host_methods::LOAD_INDEX,
    host_methods::SAVE_INDEX,
    host_methods::DELIVER,
];

/// Who wrote a conversation turn.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WireRole {
    /// The human.
    User,
    /// The agent.
    Assistant,
}

/// One message of the chat the reply answers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WireTurn {
    /// Author.
    pub role: WireRole,
    /// Text as the user saw it.
    pub text: String,
    /// True for an assistant turn already delivered remixed.
    #[serde(default)]
    pub remixed: bool,
}

fn yes() -> bool {
    true
}
fn default_timeout_ms() -> u64 {
    60_000
}

/// Remix one reply. The host decides whether to call at all (off, control and
/// treatment assignment stay with the host); `enabled = false` is an explicit
/// pass-through the module honours without touching any callback.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RemixRequest {
    /// The chat so far, oldest first.
    pub conversation: Vec<WireTurn>,
    /// The agent reply to remix.
    pub reply: String,
    /// Content region code; `None` selects the default region.
    #[serde(default)]
    pub region: Option<String>,
    /// `false` returns the reply unchanged.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Whole-request budget. On expiry the original reply is returned.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Hand the final reply to the host through `Deliver` before returning.
    #[serde(default)]
    pub deliver: bool,
}

/// What happened to the reply.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RemixResponse {
    /// The reply to send: remixed, or the original.
    pub reply: String,
    /// Whether the reply was rewritten.
    pub remixed: bool,
    /// Why the original came back unchanged, as a content-free reason code.
    #[serde(default)]
    pub skipped: Option<SkipReason>,
    /// The 0-10 rating, when a reading succeeded.
    #[serde(default)]
    pub score: Option<u8>,
    /// Jev found no fitting meme although one was wanted.
    #[serde(default)]
    pub wants_more_memes: bool,
}

/// Content-free reason the original reply was returned.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// `RemixRequest::enabled` was false.
    Disabled,
    /// The rating was below the remix threshold.
    BelowThreshold,
    /// The request budget elapsed.
    Timeout,
    /// A host callback or the engine failed; the original is returned.
    Failed,
}

/// A host callback failed. Carries no payload text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostFailure {
    /// The host serves no such capability.
    Unsupported,
    /// The host reached its backend and it failed.
    Backend,
    /// The host's own deadline elapsed.
    Timeout,
}

/// Outcome of a host callback.
pub type HostResult<T> = Result<T, HostFailure>;

/// `Complete` argument.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompleteRequest {
    /// System prompt.
    pub system: String,
    /// User prompt.
    pub user: String,
}

/// `Evaluate` argument and reply: the Jev request and response as opaque JSON
/// in the shape of `tinyinference-decisions`, so this crate links neither.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvaluateEnvelope {
    /// The request or response body.
    pub body: serde_json::Value,
}

/// `Search` argument.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchRequest {
    /// Query text.
    pub query: String,
    /// Most results wanted.
    pub max_results: u32,
}

/// One search result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchResult {
    /// Page title.
    pub title: String,
    /// Page URL.
    pub url: String,
    /// Snippet, when the provider returns one.
    #[serde(default)]
    pub snippet: Option<String>,
}

/// The scoped documents a host stores for the module. The scope (user,
/// profile) is the host's; the module never names it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexKind {
    /// The slang index.
    Slang,
    /// The learned-meme index.
    Memes,
}

/// `LoadIndex` argument.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LoadIndexRequest {
    /// Which document.
    pub kind: IndexKind,
}

/// `SaveIndex` argument.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SaveIndexRequest {
    /// Which document.
    pub kind: IndexKind,
    /// The index snapshot, as JSON text.
    pub json: String,
}

/// `Deliver` argument: the final reply.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeliverRequest {
    /// Text to deliver.
    pub reply: String,
    /// Whether it is the remixed text.
    pub remixed: bool,
}

/// Transport-level view of a remix call: `Err` is the module refusing the
/// request (malformed), never a content-quality outcome.
pub type RemixResult = Result<RemixResponse, RemixRefusal>;

/// The module refused a request before doing any work.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemixRefusal {
    /// The module is closing and accepts no new work.
    Closing,
    /// The caller is not an authenticated bus peer.
    Unauthenticated,
}

#[cfg(test)]
#[path = "remix_tests.rs"]
mod tests;
