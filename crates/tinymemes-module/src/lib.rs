//! Stateless content module. Configuration cannot admit a backend or profile.
use tinymemes_bus::{STATE_INTERFACE, STATE_PATH};

struct ContentService;
#[async_trait::async_trait]
impl tinybus::service::Interface for ContentService {
    fn name(&self) -> tinybus::InterfaceName {
        STATE_INTERFACE.try_into().unwrap()
    }
    fn members(&self) -> Vec<tinybus::MemberName> {
        vec!["Apply".try_into().unwrap()]
    }
    async fn call(
        &self,
        member: &tinybus::MemberName,
        args: serde_json::Value,
    ) -> tinybus::Result<serde_json::Value> {
        if member.as_str() != "Apply" {
            return Err(tinybus::Error::failed("unknown content operation"));
        }
        let (request,): (tinymemes_bus::ApplyRequest,) = serde_json::from_value(args)
            .map_err(|_| tinybus::Error::failed("invalid content request"))?;
        serde_json::to_value(tinymemes::state::apply(request))
            .map_err(|_| tinybus::Error::failed("invalid content response"))
    }
}
async fn setup(connection: tinybus::Connection) -> tinybus::Result<()> {
    connection
        .serve_at(STATE_PATH.try_into()?, ContentService)
        .await?;
    connection.request_name(STATE_INTERFACE).await?;
    Ok(())
}
mod exports {
    use super::setup;
    #[cfg(not(test))]
    tinybus_module::module_export! {
        setup = setup,
        worker_threads = 1,
        provides = ["ai.tinyhumans.tinymemes.ContentState"],
        methods = ["Apply"], signals = [], requires = [], optional = [], lazy = false,
    }

    #[cfg(test)]
    tinybus_module::module_export_static! {
        setup = setup,
        worker_threads = 1,
        provides = ["ai.tinyhumans.tinymemes.ContentState"],
        methods = ["Apply"], signals = [], requires = [], optional = [], lazy = false,
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
