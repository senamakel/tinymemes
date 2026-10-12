//! Portable learned content. Probabilities are data, not verification grants.
use crate::{Intent, Tier};
use serde::{Deserialize, Serialize};

/// A term reported by a researcher, before vetting.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Discovered {
    /// The term as written.
    pub term: String,
    /// Meaning and when to use it.
    pub meaning: String,
    /// Lowest tier the term suits.
    pub min_tier: Tier,
    /// Page the term was found on. Must be a page the search actually cited.
    pub source_url: String,
    /// Jev's probability that this is genuine, inoffensive regional slang
    /// with the stated meaning. `None` until verified.
    #[serde(default)]
    pub verified: Option<f64>,
}

/// Where a term came from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Shipped in the region pack.
    Curated,
    /// Learned from web research.
    Web,
}

/// One indexed term.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LearnedTerm {
    /// Region code the term belongs to.
    pub region: String,
    /// The term as written.
    pub term: String,
    /// Meaning and when to use it.
    pub meaning: String,
    /// Lowest tier the term suits.
    pub min_tier: Tier,
    /// Intents it was found for. Empty means any intent.
    pub intents: Vec<Intent>,
    /// Pages it was found on.
    pub sources: Vec<String>,
    /// How many separate searches surfaced it.
    pub seen: u32,
    /// How many remixed replies used it.
    pub used: u32,
    /// Curated or learned.
    pub origin: Origin,
    /// Jev's verification probability for web terms.
    #[serde(default)]
    pub verified: Option<f64>,
}

/// One research query that ran.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchRecord {
    /// Region code.
    pub region: String,
    /// Intent bucket the search was for.
    pub intent: Intent,
    /// The query text.
    pub query: String,
    /// Unix seconds.
    pub at: u64,
    /// Terms accepted from it.
    pub accepted: usize,
}

/// What one learning step did.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LearnReport {
    /// The query that ran.
    pub query: String,
    /// New terms added.
    pub added: usize,
    /// Existing terms corroborated.
    pub corroborated: usize,
    /// Terms rejected by the shape, blocklist, or source checks.
    pub rejected: usize,
    /// Terms Jev judged not to be genuine, inoffensive regional slang.
    pub failed_verification: usize,
}

/// One learned GIF.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LearnedMeme {
    /// Region code.
    pub region: String,
    /// Short name, unique within the region; Jev's option label.
    pub title: String,
    /// When people post it.
    pub meaning: String,
    /// Direct media link (small rendition).
    pub url: String,
    /// The GIF's page.
    pub page_url: String,
    /// Intents it fits.
    pub intents: Vec<Intent>,
    /// The host's tags.
    pub tags: Vec<String>,
    /// The host's content rating.
    pub rating: String,
    /// Jev's verification probability.
    pub verified: f64,
    /// Times it went out in a reply.
    pub used: u32,
    /// Unix seconds.
    pub added_at: u64,
}

/// One research query that ran.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MemeSearchRecord {
    /// Region code.
    pub region: String,
    /// Intent the search was for.
    pub intent: Intent,
    /// The query text.
    pub query: String,
    /// Unix seconds.
    pub at: u64,
    /// Memes added from it.
    pub added: usize,
}

/// Existing slang-index snapshot shape.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SlangSnapshot {
    /// Indexed terms.
    pub terms: Vec<LearnedTerm>,
    /// Research history.
    pub searches: Vec<SearchRecord>,
}

/// Existing meme-index snapshot shape.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct MemeSnapshot {
    /// Indexed memes.
    pub memes: Vec<LearnedMeme>,
    /// Research history.
    pub searches: Vec<MemeSearchRecord>,
}
