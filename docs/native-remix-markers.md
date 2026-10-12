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
