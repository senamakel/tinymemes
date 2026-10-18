use super::*;
use serde_json::json;
use tinystoragedrivers_core::schema::{MigrateOptions, Registry, ScopeClass};
use tinystoragedrivers_core::{
    DocumentStoreExt, MemoryStorage, Precondition, Query, Scope, ScopedStorage, StorageBackend,
    Versioned, WriteOptions,
};

fn scoped() -> ScopedStorage {
    MemoryStorage::new().for_scope(&Scope::local()).unwrap()
}

async fn seed_v1(storage: &ScopedStorage, id: &str, text: &str) {
    storage
        .documents()
        .put_with(
            LEGACY_STATE_COLLECTION,
            id,
            json!({ "json": text }),
            Precondition::Absent,
            WriteOptions::new().schema(1).created_ms(1_234),
        )
        .await
        .unwrap();
}

async fn migrated(storage: &ScopedStorage) -> Registry {
    let registry = Registry::compose([storage_schema()]).unwrap();
    registry.prepare(&MemoryStorage::new()).await.unwrap();
    registry
        .migrate(storage, &MigrateOptions::new())
        .await
        .unwrap();
    registry
}

async fn v2(storage: &ScopedStorage, id: &str) -> Option<Versioned<serde_json::Value>> {
    storage.documents().get(STATE_COLLECTION, id).await.unwrap()
}

#[test]
fn schema_declares_memes_state_as_a_tenant_collection() {
    let schema = storage_schema();
    Registry::compose([schema.clone()]).unwrap();
    let item = schema
        .items
        .iter()
        .find(|item| item.name() == "memes.state")
        .expect("memes.state is declared");
    assert_eq!(item.scope(), ScopeClass::Tenant);
    assert_eq!(schema.items.len(), 1);
}

#[test]
fn document_ids_are_the_v1_ids() {
    for kind in [IndexKind::Slang, IndexKind::Memes, IndexKind::Remixed] {
        let id = state_id(kind);
        assert_eq!(Key::of([id]).unwrap().as_str(), id);
    }
}

#[test]
fn an_object_text_becomes_the_body_itself() {
    let body = state_body(IndexKind::Slang, r#"{"terms":[],"searches":[]}"#);
    assert_eq!(body, json!({"terms": [], "searches": []}));
    let text = state_text(IndexKind::Slang, &body);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        body
    );
}

#[test]
fn the_remixed_ring_is_wrapped_because_a_body_is_an_object() {
    let body = state_body(IndexKind::Remixed, r#"["a","b"]"#);
    assert_eq!(body, json!({"ids": ["a", "b"]}));
    assert_eq!(state_text(IndexKind::Remixed, &body), r#"["a","b"]"#);
}

#[test]
fn text_that_is_not_the_expected_shape_is_kept_verbatim() {
    for (kind, text) in [
        (IndexKind::Slang, "not json"),
        (IndexKind::Slang, "[1,2]"),
        (IndexKind::Remixed, r#"{"x":1}"#),
    ] {
        let body = state_body(kind, text);
        assert_eq!(body, json!({ "unparsed": text }));
        assert_eq!(state_text(kind, &body), text);
    }
}

#[tokio::test]
async fn the_step_moves_every_v1_document_into_memes_state() {
    let storage = scoped();
    seed_v1(&storage, "slang", r#"{"terms":[],"searches":[]}"#).await;
    seed_v1(&storage, "memes", r#"{"memes":[],"searches":[]}"#).await;
    seed_v1(&storage, "remixed", r#"["m1","m2"]"#).await;
    let registry = migrated(&storage).await;
    registry.check_ready(STATE_COLLECTION).unwrap();

    let slang = v2(&storage, "slang").await.unwrap();
    assert_eq!(slang.doc, json!({"terms": [], "searches": []}));
    assert_eq!(slang.meta.schema, 2);
    assert_eq!(slang.meta.created_ms, 1_234);
    assert_eq!(
        v2(&storage, "memes").await.unwrap().doc,
        json!({"memes": [], "searches": []})
    );
    assert_eq!(
        v2(&storage, "remixed").await.unwrap().doc,
        json!({"ids": ["m1", "m2"]})
    );
    let legacy = storage
        .documents()
        .query_all(LEGACY_STATE_COLLECTION, &Query::all())
        .await
        .unwrap();
    assert!(legacy.is_empty(), "the v1 collection is drained");
}

#[tokio::test]
async fn the_step_is_a_no_op_without_v1_data_and_idempotent() {
    let storage = scoped();
    let registry = migrated(&storage).await;
    registry.check_ready(STATE_COLLECTION).unwrap();
    registry
        .migrate(&storage, &MigrateOptions::new())
        .await
        .unwrap();
    assert!(v2(&storage, "slang").await.is_none());
}

#[tokio::test]
async fn the_step_preserves_document_expiry_metadata() {
    let storage = MemoryStorage::with_clock(std::sync::Arc::new(|| 1_234))
        .for_scope(&Scope::local())
        .unwrap();
    storage
        .documents()
        .put_with(
            LEGACY_STATE_COLLECTION,
            "slang",
            json!({ "json": r#"{"terms":[],"searches":[]}"# }),
            Precondition::Absent,
            WriteOptions::new().schema(1).expires_ms(9_999),
        )
        .await
        .unwrap();

    migrated(&storage).await;

    assert_eq!(v2(&storage, "slang").await.unwrap().meta.expires_ms, Some(9_999));
}

#[tokio::test]
async fn the_step_keeps_unparseable_and_malformed_v1_documents() {
    let storage = scoped();
    seed_v1(&storage, "slang", "not json").await;
    storage
        .documents()
        .put_with(
            LEGACY_STATE_COLLECTION,
            "memes",
            json!({ "other": 1 }),
            Precondition::Absent,
            WriteOptions::new().schema(1),
        )
        .await
        .unwrap();
    migrated(&storage).await;
    assert_eq!(
        v2(&storage, "slang").await.unwrap().doc,
        json!({"unparsed": "not json"})
    );
    assert_eq!(
        v2(&storage, "memes").await.unwrap().doc,
        json!({"unparsed": r#"{"other":1}"#})
    );
}

#[test]
fn an_object_with_an_unparsed_field_survives_a_round_trip() {
    for kind in [IndexKind::Slang, IndexKind::Memes] {
        let text = r#"{"unparsed":"hello","terms":[]}"#;
        let body = state_body(kind, text);
        assert_eq!(body, json!({ "unparsed": text }));
        assert_eq!(state_text(kind, &body), text);
    }
}
