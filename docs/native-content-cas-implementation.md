# Native catalog usage and removal

## Scope and current stage

Canonical base is TinyMemes dc028f3531530bc06f558fa3ce7a69f3bfc9e935.
The operation contract, compiled-module Get-only baseline, and regression sources
are staged. No content baseline has executed yet and no production CAS loop is
implemented. The existing marker implementation remains unchanged.

TinyBus canonical main 40814a8279ff71b8fb411f9f29362c8ec0be51e2 contains both
PR59 callback drain/cancellation accounting and PR60 native SDK reload correction.
Its SDK execution-context behavior matches the prior tested 5256 candidate;
the newer host lifecycle behavior requires fresh canonical tests. This does not
establish OpenHuman reset/drain or a released module.

Canonical TSD d2f85b2 still pins TinyBus 1f255765. This development branch now
pins the public, not-yet-merged owning update
a981da30c29c50b89f348ac1376fc7b9469bcbec and canonical TinyBus40814 throughout.
Historical Cargo replacements are removed. Both locks and fresh metadata/proofs
still need regeneration and execution after the dependency owner's Cargo handoff.
This graph is an explicit unpublished TSD dependency, not canonical availability.
No aliases, copied adapter, or host shim are used.

## Contract

Confidential ContentUsage1 provides RecordSlangUsage(region, text),
RecordMemeUsage(media_url), and RemoveMeme(region, title). Inputs contain only
business arguments. They cannot supply a stored snapshot, expected revision,
document selector, collection, destination, backend URL, or research grant.

Each operation retains its original authenticated confidential sender and uses
the fixed private document callback path and content-state key. Authority comes
from the host-selected exact loaded module callback binding, never the arguments.

The stored format1 envelope is the full slang and meme catalog snapshots,
including every record field and both research histories, with canonical decimal
business_revision. It is separate from remix-state: catalog updates never read,
write, copy, or claim ownership of remix markers. The format accepts the owning
serializer's canonical record shape. Decode followed by exact Value roundtrip
comparison rejects unknown nested data and default-normalized missing fields.
It is not a generic classic JSON importer or schema migration.

Existing duplicate catalog entries and their order are legal. Slang usage
increments each matching stored regional entry once using existing token rules.
Meme usage increments only the first exact URL match. Removal deletes every
exact region/title match. Verification scores and source metadata remain data;
no new research admission, deduplication, or validation policy is introduced.

## Implementation sequence after genuine baseline

1. Decode the member's deny-unknown-fields request and retain actual sender.
2. Get content-state afresh through the admitted native callback. Missing alone
   yields the empty catalog; malformed/unsupported state refuses.
3. Validate format1, canonical decimal u64 revision, exact full record shape,
   and returned native document identity before invoking the owning reducer.
4. Supply that freshly read catalog and internal revision to the existing pure
   reducer. Caller data does not select a snapshot. Reducer overflow is returned
   without Put; unchanged catalog returns without Put or version increment.
5. Serialize the changed canonical catalog and condition Put on the native
   document Version (Absent only for genuinely absent state).
6. Retry only native Put Conflict, with at most eight fresh Get/apply/Put attempts.
   Storage refusal and outer transport errors exit once. Native Version and
   business revision are distinct.

Usage increments are not idempotent. A native-port unavailable acknowledgment
can follow a real committed increment. The module must not retry it; a caller
retry can count again. No rollback, exactly-once delivery, or outer lost-packet
proof is claimed.

## Regression sources

The excluded proof workspace reuses its actual compiled, SHA-256-admitted SDK
module fixture and captured fixed-collection Memory/SQLite ports. It does not
link TinyMemes implementation or compute expected reducer output on the host.

Sources cover complete metadata/history/duplicate order preservation, untouched
marker documents and sibling scopes, genuine SQLite database-file reopen,
missing no-op, corrupt nested data, counter and journal exhaustion, two private
module instances competing on real native Version CAS, bounded injected conflicts,
nonconflict refusal, actual Put followed by port acknowledgment loss, native
stale-grant mutation refusal, plain/unbound caller refusal, and unknown selectors.
Fault-injection controls are separate from the genuine conditional-write race.
Snapshot Get may succeed for stale grants; mutation must refuse and preserve
native version/data. None of these unexecuted sources is RED/GREEN evidence yet.

Learning/research, historical import, publication/bootstrap authority, broader
engine persistence, reset, and OpenHuman adapters remain outside this slice.
