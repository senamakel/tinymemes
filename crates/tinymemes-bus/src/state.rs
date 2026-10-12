//! Typed content transformations, distinct from a future admitted storage API.
use crate::{MemeSnapshot, SlangSnapshot};
use serde::{Deserialize, Serialize};

/// Caller-supplied content. Revision is a comparison token, not durable authority.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ContentState {
    /// Revision of this supplied content value.
    pub revision: u64,
    /// Existing portable slang snapshot.
    pub slang: SlangSnapshot,
    /// Existing portable meme snapshot.
    pub memes: MemeSnapshot,
    /// Reply IDs already remixed; these IDs do not select a profile or scope.
    pub remixed: Vec<String>,
}

/// An owner-defined operation on explicit content, with no network or I/O.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentOperation {
    /// Count matching terms once each, using existing regional token matching.
    RecordSlangUsage {
        /// Content region code; not a tenant selector.
        region: String,
        /// Text whose matching terms were used.
        text: String,
    },
    /// Count the first learned meme with this media URL, as the existing index does.
    RecordMemeUsage {
        /// Learned media URL used in a reply.
        url: String,
    },
    /// Remove all exact region/title matches, preserving the existing semantics.
    RemoveMeme {
        /// Content region code; not a tenant selector.
        region: String,
        /// Exact learned title.
        title: String,
    },
    /// Mark a reply ID once. An existing marker is a no-op.
    MarkRemixed {
        /// Content deduplication key; not storage authority.
        reply_id: String,
    },
}

/// Conditional comparison of explicitly supplied content. This is NOT store CAS.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ApplyRequest {
    /// Complete content supplied by the caller.
    pub state: ContentState,
    /// Expected revision of that supplied value.
    pub expected_revision: u64,
    /// Transformation to perform.
    pub operation: ContentOperation,
}

/// Refusal before returning any transformed content.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentError {
    /// The supplied comparison token does not match the supplied snapshot.
    RevisionConflict {
        /// Requested comparison value.
        expected: u64,
        /// Revision of the supplied content.
        actual: u64,
    },
    /// A changed snapshot cannot increment its revision.
    RevisionExhausted,
    /// A usage counter cannot be incremented without overflow.
    CounterExhausted,
}
/// Content outcome carried inside a successful TinyBus reply.
pub type ApplyResult = Result<ContentState, ContentError>;
