# Persistent remix-marker staging

This branch depends on **unmerged** TinyStorageDrivers PR 27 at
`7c5596f37e5d5440fbfa3a049a3ca98571885584`. It is not a released contract.
TinyBus is pinned to canonical `1f2557657db97470cd4efb7e1fc2f374f36fdd22`.
No TinyBus PR 59 lifecycle behavior is consumed or claimed.

The new operation-only marker contract carries one business `reply_id`.
It carries no snapshot, revision precondition, scope, URL, collection or
callback destination. An optional host client accepts only the canonical
opaque `HostCallbackProxy`; default bus features do not enable module loading.
The compiled module retains the original confidential authenticated sender
across its awaited fixed-collection document callback. Neither module config
nor caller strings select native authority.

## Deliberate baseline

The staged module authenticates and decodes the operation, then executes a
fresh native Get through the captured callback. Native refusal is returned as
a sanitized storage kind. Admitted reads deliberately return Unavailable:
there is no persistent CAS implementation yet, and no fake-map substitute.

All newly staged tests are **unexecuted**. No RED or GREEN is claimed.
The excluded proof runner loads the actual SDK-built module with its verified
hash and captures real Memory/SQLite document ports. Cases cover reopen/scope
behavior, ordinary/unbound caller and selector refusal, malformed native state,
stale native grants, and a victim binding A request to same-artifact instance B
on the same broker. The last case must fail callback admission for B rather
than inherit an unrelated proxy test as proof.

## Subsequent production boundary

After actual behavioral RED, the owner module will read and validate native
state, apply its existing owning content reducer, and CAS absent/native-version
writes. Only native Conflict may trigger bounded reread/retry. Content revision
and native document version remain distinct; duplicates require an authorized
fresh Get. Malformed state is refused, never replaced by defaults. A lost
acknowledgment is uncertain and does not imply rollback.

Deterministic competing-read fixtures interpose a two-reader barrier over real native documents, retaining native CAS and state rather than imitating persistence. This work does
not establish research verification, learning/import authority, session drain,
reset behavior, released module availability or host migration.

## First actual baseline

Cancel-aware run 53711 exited 101. The actual SDK module compiled, and all ten
proof cases executed: one unauthorized/selector control passed; eight behavior
cases failed on the deliberately returned Unavailable (two reopen/scope, three
competing-read CAS, one corrupt-state, two stale-grant mutation cases). Native
Get is allowed to read an immutable stale actor snapshot; its mutation must
still be refused once the operation is implemented. Those failures do not
prove the driver fences reads.

The remaining wrong-instance case failed during fixture setup, not at callback
admission: canonical TinyBus reserves the artifact manifest bus name in the
shared broker, so a second same-artifact live instance is rejected as "module
bus name is already owned", even with separate ModuleHost objects. The public
canonical loader exposes no instance-name override. This exact replay proof
remains blocked and unproved; no manifest spoofing or loader relaxation is
introduced here. The test is retained to identify the missing composition
capability. Production CAS implementation is pending the required authority
proof disposition.

Baseline log: `target/gates/remix-marker-red.log`. Earlier statements that all
staged tests were unexecuted describe the pre-run checkpoint only.

## Canonical instance-restriction disposition

The simultaneous instance requirement was replaced by explicit canonical
refusal and sequential replacement controls. Run 64658 exited 101: duplicate
load/no-effects and unauthorized/selector controls passed; the eight intended
Unavailable behavior failures remained. Sequential B setup failed with
"module initialization failed" after A.shutdown, so it was not authentication
proof. Run 49264 also exited 101 with the same disposition after normal
shutdown plus dropping A's ModuleHost while retaining its binding/registration.
The callback refusal assertion has still not executed for a new B instance.

SQLite fixtures now use an explicit `markers.db` file. Canonical
SqliteStorage::open chooses File mode by extension and otherwise chooses
Directory mode with storage.db, so the original directory use was supported,
not itself a setup failure. The cold reopen test scopes and drops the first
module and backend, then opens a fresh SqliteStorage from that same file.
Its cold-reopen assertions remain unexecuted until the marker operation exists.

The current module also requests the stateless ContentState global alias;
canonical stopped-host cleanup does not detach every retained router alias.
The replacement initialization failure is under investigation. No alias is
manually removed, no manifest is spoofed, and no loader override is added.
