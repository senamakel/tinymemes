//! The module's storage declaration: what a host composes into its schema
//! registry and generates this module's storage grant from.
//!
//! The module itself never touches a collection: it asks the host to load and
//! save one JSON text per [`IndexKind`] (`LoadIndex` / `SaveIndex`). The host
//! keeps those texts in `memes.state`, one document per kind, as the *parsed*
//! text (v1 kept the text as a string in `{"json": "<text>"}` in
//! `tinymemes_documents`). [`state_body`] and [`state_text`] are the two
//! directions of that mapping, so the host and the migration step agree on it.
use crate::IndexKind;
use serde_json::{Value, json};
use tinystoragedrivers_core::schema::{
    Owner, Schema, ScopeClass, SourceRecord, Step, TargetRecord,
};
use tinystoragedrivers_core::{Key, Result};

/// The v2 collection holding the learned-state documents.
pub const STATE_COLLECTION: &str = "memes.state";
/// The v1 collection the host kept them in, one `{"json": "<text>"}` body each.
pub const LEGACY_STATE_COLLECTION: &str = "tinymemes_documents";

/// Body field holding the remixed ids (a body must be an object, the ring is an
/// array).
const IDS: &str = "ids";
/// Body field holding text that is not the shape its kind expects, kept
/// verbatim so a migration never destroys it. The module treats such a
/// document as a fresh start, as it did before.
const UNPARSED: &str = "unparsed";

/// Declare `memes.state` and the step that brings `tinymemes_documents` into it.
///
/// # Panics
///
/// Never in practice: the owner name is a constant that satisfies the grammar.
#[must_use]
pub fn storage_schema() -> Schema {
    Schema::new(Owner::new("memes").expect("`memes` is a valid owner"))
        .collection(STATE_COLLECTION, 2, ScopeClass::Tenant)
        .step(
            Step::new("state_v2", STATE_COLLECTION, 1, 2, migrate_state)
                .source(LEGACY_STATE_COLLECTION),
        )
}

/// The document id of `kind` (unchanged from v1; a plain word is its own
/// [`Key`]).
#[must_use]
pub fn state_id(kind: IndexKind) -> &'static str {
    match kind {
        IndexKind::Slang => "slang",
        IndexKind::Memes => "memes",
        IndexKind::Remixed => "remixed",
    }
}

/// The v2 body for the text the module saved for `kind`.
///
/// A JSON object is the body; the remixed ring (an array of ids) becomes
/// `{"ids": [...]}`. Anything else, including an object whose own `unparsed` field is a string,
/// is kept as `{"unparsed": "<text>"}`.
#[must_use]
pub fn state_body(kind: IndexKind, text: &str) -> Value {
    body_for(state_id(kind), text)
}

/// The text the module loads back from a v2 body: the inverse of [`state_body`].
#[must_use]
pub fn state_text(kind: IndexKind, body: &Value) -> String {
    if let Some(text) = body.get(UNPARSED).and_then(Value::as_str) {
        return text.to_owned();
    }
    match (kind, body.get(IDS)) {
        (IndexKind::Remixed, Some(ids)) => ids.to_string(),
        _ => body.to_string(),
    }
}

fn body_for(id: &str, text: &str) -> Value {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Array(ids)) if id == state_id(IndexKind::Remixed) => json!({ IDS: ids }),
        // An object carrying a string `unparsed` field would be read back as the
        // fallback marker by `state_text`, so it is wrapped like any other text.
        Ok(Value::Object(fields))
            if id != state_id(IndexKind::Remixed)
                && !fields.get(UNPARSED).is_some_and(Value::is_string) =>
        {
            Value::Object(fields)
        }
        _ => json!({ UNPARSED: text }),
    }
}

/// v1 (`{"json": "<text>"}`) to v2 (the parsed text), keeping ids, creation
/// time and expiry.
fn migrate_state(batch: Vec<SourceRecord>) -> Result<Vec<TargetRecord>> {
    batch
        .into_iter()
        .map(|record| {
            let body = match record.body.get("json").and_then(Value::as_str) {
                Some(text) => body_for(&record.id, text),
                None => json!({ UNPARSED: record.body.to_string() }),
            };
            let id = Key::of([record.id.as_str()])?;
            Ok(TargetRecord::new(STATE_COLLECTION, id.as_str(), body)
                .created_ms(record.meta.created_ms)
                .expires_ms(record.meta.expires_ms))
        })
        .collect()
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
