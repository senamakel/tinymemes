//! The closed set of chat intents Jev chooses between.

use serde_json::{Value, json};
pub use tinymemes_bus::Intent;

pub(crate) fn criteria() -> std::collections::BTreeMap<String, Option<Value>> {
    Intent::ALL
        .into_iter()
        .map(|i| (i.as_str().to_owned(), Some(json!(i.description()))))
        .collect()
}
