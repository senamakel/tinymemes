use super::*;

#[test]
fn every_native_storage_kind_maps_to_its_contract_mirror() {
    let json = |kind: StorageKind| serde_json::to_value(kind).unwrap();
    for kind in [
        StorageKind::NotFound,
        StorageKind::Conflict,
        StorageKind::AlreadyExists,
        StorageKind::Unsupported,
        StorageKind::InvalidInput,
        StorageKind::Serialization,
        StorageKind::Crypto,
        StorageKind::Unavailable,
        StorageKind::Backend,
        StorageKind::Fenced,
        StorageKind::ScopeGenerationRejected,
    ] {
        assert_eq!(serde_json::to_value(marker_kind(kind)).unwrap(), json(kind));
    }
}
