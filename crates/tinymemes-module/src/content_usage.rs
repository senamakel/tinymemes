//! Catalog operations through this request's authenticated private callback.
//! This baseline intentionally stops after the actual native Get.
use tinymemes_bus::{
    CONTENT_DOCUMENT_KEY, CONTENT_USAGE_INTERFACE, ContentUsageFailure, MARKER_DOCUMENT_PATH,
};
use tinystoragedrivers_bus::{DocumentClient, GetRequest};

pub(crate) struct ContentUsageService {
    pub(crate) connection: tinybus::Connection,
}
#[async_trait::async_trait]
impl tinybus::Interface for ContentUsageService {
    fn name(&self) -> tinybus::InterfaceName {
        CONTENT_USAGE_INTERFACE.try_into().unwrap()
    }
    fn members(&self) -> Vec<tinybus::MemberName> {
        ["RecordSlangUsage", "RecordMemeUsage", "RemoveMeme"]
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
        match member.as_str() {
            "RecordSlangUsage" => {
                let _: (tinymemes_bus::SlangUsageRequest,) = serde_json::from_value(args)
                    .map_err(|_| tinybus::Error::failed("invalid catalog request"))?;
            }
            "RecordMemeUsage" => {
                let _: (tinymemes_bus::MemeUsageRequest,) = serde_json::from_value(args)
                    .map_err(|_| tinybus::Error::failed("invalid catalog request"))?;
            }
            "RemoveMeme" => {
                let _: (tinymemes_bus::RemoveMemeRequest,) = serde_json::from_value(args)
                    .map_err(|_| tinybus::Error::failed("invalid catalog request"))?;
            }
            _ => return Err(tinybus::Error::failed("unknown catalog operation")),
        }
        let documents = DocumentClient::new(self.connection.proxy(
            caller.as_str(),
            MARKER_DOCUMENT_PATH,
            tinystoragedrivers_bus::INTERFACE,
        )?)?;
        let failure = match documents
            .get(GetRequest {
                id: CONTENT_DOCUMENT_KEY.into(),
            })
            .await?
        {
            Err(error) => ContentUsageFailure::Storage(error.kind),
            Ok(_) => ContentUsageFailure::Unavailable,
        };
        Ok(serde_json::to_value(Result::<(), _>::Err(failure))?)
    }
}
