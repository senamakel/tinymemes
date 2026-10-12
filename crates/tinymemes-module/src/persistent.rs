//! Authenticated callback routing baseline; persistent mutation is deliberately absent.
use tinymemes_bus::{
    MARKER_DOCUMENT_KEY, MARKER_DOCUMENT_PATH, MARKER_INTERFACE, MarkerFailure, MarkerRequest,
};

pub(crate) struct PersistentService {
    pub(crate) connection: tinybus::Connection,
}
#[async_trait::async_trait]
impl tinybus::Interface for PersistentService {
    fn name(&self) -> tinybus::InterfaceName {
        MARKER_INTERFACE.try_into().unwrap()
    }
    fn members(&self) -> Vec<tinybus::MemberName> {
        ["MarkRemixed", "WasRemixed"]
            .into_iter()
            .map(|name| name.try_into().unwrap())
            .collect()
    }
    fn requires_confidential(&self, _: &tinybus::MemberName) -> bool {
        true
    }
    async fn call(
        &self,
        _: &tinybus::MemberName,
        _: serde_json::Value,
    ) -> tinybus::Result<serde_json::Value> {
        Err(tinybus::Error::failed(
            "authenticated confidential caller required",
        ))
    }
    async fn call_with_context(
        &self,
        member: &tinybus::MemberName,
        args: serde_json::Value,
        context: &tinybus::CallContext,
    ) -> tinybus::Result<serde_json::Value> {
        let caller = context
            .authenticated_sender()
            .filter(|_| context.is_confidential())
            .ok_or_else(|| tinybus::Error::failed("authenticated confidential caller required"))?
            .clone();
        if !matches!(member.as_str(), "MarkRemixed" | "WasRemixed") {
            return Err(tinybus::Error::failed("unknown marker operation"));
        }
        let (_request,): (MarkerRequest,) = serde_json::from_value(args)
            .map_err(|_| tinybus::Error::failed("invalid marker request"))?;
        let client = tinystoragedrivers_bus::DocumentClient::new(self.connection.proxy(
            caller.as_str(),
            MARKER_DOCUMENT_PATH,
            tinystoragedrivers_bus::INTERFACE,
        )?)?;
        // Exercise real admission even in the deliberately unimplemented baseline.
        // No caller snapshot, default state, or mutation can bypass this fresh read.
        let outcome = client
            .get(tinystoragedrivers_bus::GetRequest {
                id: MARKER_DOCUMENT_KEY.into(),
            })
            .await?;
        let failure = match outcome {
            Err(error) => MarkerFailure::Storage(error.kind),
            Ok(_) => MarkerFailure::Unavailable,
        };
        Ok(serde_json::to_value(Result::<(), _>::Err(failure))?)
    }
}
