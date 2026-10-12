# Native catalog usage and removal

## Scope and current stage

Canonical base is TinyMemes dc028f3531530bc06f558fa3ce7a69f3bfc9e935.
The operation contract and bounded native CAS implementation are complete at
592b6db. The existing marker implementation remains unchanged. Broader owner
gates and publication are pending; the authentic baseline and candidate results
are recorded below.

TinyBus canonical main 40814a8279ff71b8fb411f9f29362c8ec0be51e2 contains both
PR59 callback drain/cancellation accounting and PR60 native SDK reload correction.
Its SDK execution-context behavior matches the prior tested 5256 candidate;
the newer host lifecycle behavior requires fresh canonical tests. This does not
establish OpenHuman reset/drain or a released module.

Canonical TSD d2f85b2 still pins TinyBus 1f255765. This development branch now
pins the public, not-yet-merged owning update
a981da30c29c50b89f348ac1376fc7b9469bcbec and canonical TinyBus40814 throughout.
Historical Cargo replacements are removed. Both locks were regenerated and inspected: root metadata resolves exactly one
TinyBus and one SDK at40814; excluded proof resolves exactly one TinyBus and no
SDK. All TSD packages resolve the exact public a981 commit. No external path
package or replacement is present.
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

## Implemented operation sequence

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
native version/data. The source-only stage was not evidence. Actual executions are recorded below.

## Executed baseline and candidate

At graph9df53aa, cancel-aware marker6837 exited0: all18 existing real compiled
marker cases passed on canonical Bus40814/public TSD28a981. Content19209 exited101:
all17 new cases ran, two admission controls passed and fifteen failed on the
Get-only stub's Unavailable. No compiler/setup failure was counted as RED.
The replacement negative reached exact NotAttested with zero native calls;
its failure was the admitted fresh B no-op positive returning Unavailable.

After production592b6db, cancel-aware70124 exited0. The actual SDK module rebuilt
and the complete authentic proof passed35/0failed/0ignored (17catalog+18marker,
not counted twice). All baseline assertions were retained. This includes true
SQLite file reopen, Memory/SQLite native stale-write refusal, real native
competing CAS, full-field preservation and native-port lost acknowledgment.

Logs under this worktree's target/gates: canonical-marker-controls.log,
content-native-red.log, content-module-green-build.log, content-native-green.log.
These local native proofs are not external Mongo/S3, historical import, or
OpenHuman composition evidence.

Learning/research, historical import, publication/bootstrap authority, broader
engine persistence, reset, and OpenHuman adapters remain outside this slice.
