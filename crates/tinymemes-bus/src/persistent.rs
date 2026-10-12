//! Operation-only persistent marker contract. Native authority is not wire data.
use serde::{Deserialize, Serialize};

/// Persistent marker interface.
pub const MARKER_INTERFACE: &str = "ai.tinyhumans.tinymemes.RemixMarkers1";
/// Persistent marker object path.
pub const MARKER_PATH: &str = "/ai/tinyhumans/tinymemes/RemixMarkers";
/// Fixed private document callback path used by the module.
pub const MARKER_DOCUMENT_PATH: &str = "/ai/tinyhumans/tinymemes/Documents";
/// Fixed document key; callers cannot select documents or collections.
pub const MARKER_DOCUMENT_KEY: &str = "remix-state";

/// Business identifier, never an authority selector or persisted snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkerRequest {
    /// Reply whose marker is read or recorded.
    pub reply_id: String,
}
/// Result of a committed marker operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkRemixedReply {
    /// Whether this operation added a marker.
    pub changed: bool,
    /// Owning content revision, distinct from the native document version.
    pub business_revision: u64,
}
/// Content-free failures. Native source chains never cross the ABI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum MarkerFailure {
    /// Native storage refused the operation.
    Storage(tinystoragedrivers_bus::StorageKind),
    /// Persisted state cannot be decoded or validated.
    CorruptState,
    /// Owning content operation refused.
    Content(crate::ContentError),
    /// Bounded native conflict retries were exhausted.
    ConflictExhausted,
    /// Persistent implementation is unavailable.
    Unavailable,
}
/// Persistent operation outcome, separate from transport admission.
pub type MarkerOutcome<T> = Result<T, MarkerFailure>;

/// A client constructed only from the host-selected callback capability.
#[cfg(feature = "host-callback")]
#[derive(Clone, Debug)]
pub struct RemixMarkerClient(tinybus::module::HostCallbackProxy);
#[cfg(feature = "host-callback")]
impl RemixMarkerClient {
    /// Bind an already-authorized, exact-interface callback proxy.
    pub fn new(proxy: tinybus::module::HostCallbackProxy) -> tinybus::Result<Self> {
        if proxy.interface().as_str() != MARKER_INTERFACE {
            return Err(tinybus::Error::failed("marker interface mismatch"));
        }
        Ok(Self(proxy))
    }
    /// Record a marker using fresh native state inside the compiled module.
    pub async fn mark_remixed(
        &self,
        request: MarkerRequest,
    ) -> tinybus::Result<MarkerOutcome<MarkRemixedReply>> {
        self.0.call_confidential("MarkRemixed", (request,)).await
    }
    /// Read a marker using fresh native state inside the compiled module.
    pub async fn was_remixed(
        &self,
        request: MarkerRequest,
    ) -> tinybus::Result<MarkerOutcome<bool>> {
        self.0.call_confidential("WasRemixed", (request,)).await
    }
}

#[cfg(test)]
#[path = "persistent_tests.rs"]
mod tests;
