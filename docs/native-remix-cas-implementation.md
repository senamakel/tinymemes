# Bounded marker CAS implementation note

Status: implemented and reviewed at ff82038. The original 18-case authentic
baseline passed three admission controls and failed fifteen behavior cases;
the implementation passed all eighteen with the explicitly unpublished runtime
path override. Final public-only graph acceptance is recorded separately.
The following sequence describes the implemented boundary, not host migration.

## Native state and reducer boundary

Use a dedicated private marker document, not the full learned-index snapshot:
`remix-state` in the already-captured collection. The module, not the caller,
owns a strict format-version envelope, business revision and remixed ID list.
No research verification data is introduced or accepted by these operations.
The existing owning `state::apply(MarkRemixed)` can receive a locally assembled
ContentState with this revision/list and empty unrelated indexes. Only its
resulting marker fields are persisted. This mechanically reuses duplicate and
revision-exhaustion semantics without exposing caller snapshots.

Keep the stored revision portable as a strict canonical decimal string:
JSON/native Mongo BSON need not accept unsigned integers beyond i64::MAX.
Parse only canonical decimal u64 values (no signs, whitespace or leading-zero
aliases); serialize zero as "0". The public result remains a business u64.
Native document Version is an independent opaque CAS token, never derived from
that business revision. Memory/SQLite are the current actual proof targets;
this representation choice alone does not establish Mongo production support.

Decode the native envelope strictly, including the format version and field
set. Missing document alone means empty markers at business revision zero.
Malformed existing data, unknown format, invalid revision or duplicate stored
markers means CorruptState and no mutation. Preserve current pure reducer
identifier semantics, including arbitrary content strings; identifiers confer
no authority. Do not fabricate migration/import receipts or source ownership.

## Operation sequence

1. Require the original CallContext confidential authenticated sender; retain
   the actual BusName across awaits. Decode only MarkerRequest, refusing unknown
   fields before native I/O.
2. Construct the fixed-interface DocumentClient on the retained module
   Connection, directed to that original sender and fixed callback path.
3. Get the fixed document freshly. Propagate storage refusal by sanitized kind;
   keep transport failures outer. Validate existing data before any default,
   no-op return or mutation.
4. WasRemixed returns the presence from that admitted read. MarkRemixed invokes
   the owning pure reducer with the module-derived business revision. Duplicate
   markers return changed=false only after that fresh admitted read, without
   Put. The business revision stays unchanged even at u64::MAX.
5. Changed state uses native Precondition::Absent for a missing document, or
   Precondition::Version for the returned native Version. No unconditional Put.
   A successful native Put permits the successful committed result.
6. Only native StorageKind::Conflict triggers a bounded fresh Get/reapply loop
   (maximum eight attempts). AlreadyExists, Fenced, Unsupported,
   unavailable transport, malformed state and content refusal never retry as
   conflicts. Exhaustion returns ConflictExhausted. Lost acknowledgments remain
   uncertain: an outer transport failure makes no rollback claim.

## Additional executable tests after baseline disposition

Preserve existing actual SDK/native fixtures and add:

- strict malformed envelope matrix: missing/wrong format, invalid revision,
  wrong field type, unknown fields and duplicate stored IDs. Assert same native
  version/content after refusal;
- revision u64::MAX: existing duplicate returns unchanged MAX without Put;
  new marker returns Content(RevisionExhausted), preserving native bytes/version;
- real port wrapper that returns Conflict for the first bounded attempts and
  counts fresh Get/conditional Put; assert eventual merge or exact exhaustion;
- real native-port refusal interposition proving non-Conflict errors and a
  native-port lost-acknowledgment error do not trigger hidden retries or defaults;
  this is not proof of a dropped outer TinyBus acknowledgment;
- maintain barrier-controlled independent-handle conflicts and duplicate-only
  one-commit assertions. SQLite distinct contenders are executed; SQLite
  duplicate contender parity remains a possible unproved addition;
- preserve fresh private-broker scope positives, exact duplicate-name refusal,
  corrected sequential replacement admission, stale native grant and malformed
  state controls without changing their authority expectations.

See native-remix-markers.md for exact executed test chronology. Learning/research operations,
legacy import, generation reset and callback drain remain outside this slice.
