//! The module's view of the host: the callbacks served at `HOST_INTERFACE`.
use std::sync::Arc;

use async_trait::async_trait;
use tinymemes_bus::{
    CompleteRequest, DeliverRequest, EvaluateEnvelope, HOST_METHODS, HostFailure, HostResult,
    IndexKind, LoadIndexRequest, SaveIndexRequest, SearchRequest, SearchResult, host_methods,
};

/// Everything the module asks of the host. A mock implements this in tests.
#[async_trait]
pub(crate) trait Host: Send + Sync {
    async fn complete(&self, request: CompleteRequest) -> HostResult<String>;
    async fn evaluate(&self, request: serde_json::Value) -> HostResult<serde_json::Value>;
    async fn search(&self, request: SearchRequest) -> HostResult<Vec<SearchResult>>;
    async fn load_index(&self, kind: IndexKind) -> HostResult<Option<String>>;
    async fn save_index(&self, kind: IndexKind, json: String) -> HostResult<()>;
    async fn deliver(&self, request: DeliverRequest) -> HostResult<()>;
}

/// The caller's host over the bus. The destination is the request's
/// authenticated sender, never configuration or request data.
pub(crate) struct BusHost(pub(crate) tinybus::Proxy);

impl BusHost {
    async fn ask<A, R>(&self, member: &'static str, args: A) -> HostResult<R>
    where
        A: serde::Serialize + Send,
        R: serde::de::DeserializeOwned,
    {
        debug_assert!(HOST_METHODS.contains(&member));
        // A transport failure or an undecodable reply is the host failing.
        match self.0.call::<HostResult<R>>(member, (args,)).await {
            Ok(result) => result,
            Err(error) => {
                tracing::debug!(member, error = %error, "[tinymemes] host callback failed");
                Err(HostFailure::Backend)
            }
        }
    }
}

#[async_trait]
impl Host for BusHost {
    async fn complete(&self, request: CompleteRequest) -> HostResult<String> {
        self.ask(host_methods::COMPLETE, request).await
    }
    async fn evaluate(&self, request: serde_json::Value) -> HostResult<serde_json::Value> {
        let reply: EvaluateEnvelope = self
            .ask(host_methods::EVALUATE, EvaluateEnvelope { body: request })
            .await?;
        Ok(reply.body)
    }
    async fn search(&self, request: SearchRequest) -> HostResult<Vec<SearchResult>> {
        self.ask(host_methods::SEARCH, request).await
    }
    async fn load_index(&self, kind: IndexKind) -> HostResult<Option<String>> {
        self.ask(host_methods::LOAD_INDEX, LoadIndexRequest { kind })
            .await
    }
    async fn save_index(&self, kind: IndexKind, json: String) -> HostResult<()> {
        self.ask(host_methods::SAVE_INDEX, SaveIndexRequest { kind, json })
            .await
    }
    async fn deliver(&self, request: DeliverRequest) -> HostResult<()> {
        self.ask(host_methods::DELIVER, request).await
    }
}

pub(crate) type SharedHost = Arc<dyn Host>;
