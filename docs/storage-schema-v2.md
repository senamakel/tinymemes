# Storage on the v2 schema

`tinymemes-bus` (feature `storage`, off by default; the host enables `tinymemes-bus/storage`) exports `tinymemes_bus::storage::storage_schema()`,
`storage_schema() -> tinystoragedrivers_core::Schema` (the ports-only storage
core, no I/O). A host composes it with
`Registry::compose`, and the module's storage grant is generated from its items.

The storage ports pin `tinystoragedrivers` v1.6.0. Its published schema types,
composition, migration runner, and schema conformance sources are unchanged
from v1.0.0. The v1.6 core's in-memory native-watch changes are behind its
off-by-default `test-util` feature; `tinymemes-bus/storage` enables only the
ports-only core with default features disabled. The bus adds storage-needs
declarations, while its legacy `DocumentClient` remains available behind the
explicit `client` feature. The module enables that client explicitly. Its
standalone manifest pins SDK v0.3.3 and patches the canonical Git source to the
same nested SDK checkout; its compatible v0.3 range also lets a host workspace
unify it with an additive TinyBus release.

| v1 | v2 | Scope class | Change |
|---|---|---|---|
| `tinymemes_documents` (`slang`, `memes`, `remixed`; body `{"json": "<text>"}`) | `memes.state` (same ids; owner `memes`, version 2) | Tenant | the `json` text is parsed into the body |

The module never reads a collection: it asks the host for one JSON text per
`IndexKind` (`LoadIndex` / `SaveIndex`), and the host stores them. The same
mapping is exported both ways, so host and migration agree on it:

- `state_body(kind, text)`: an object text is the body itself; the remixed ring
  (an array of ids) becomes `{"ids": [...]}` because a body is an object; text
  that is not the shape its kind expects is kept verbatim as `{"unparsed":
  "<text>"}`, so no migration destroys data. The module reads such a document as
  a fresh start, as before.
- `state_text(kind, body)`: the inverse, giving the text `LoadIndex` returns.
- `state_id(kind)`: `slang`, `memes`, `remixed`.

The step `state_v2` (`tinymemes_documents` -> `memes.state`, version 1 to 2) is
pure: it keeps each record's id, creation time and expiry. The host runs it with
`Registry::migrate`.

The remix marker document (`remix-state`) lives in the caller's private document
port, whose collection the host chooses; it already has the snake_case,
object-only shape and needs no step.

The migration tests cover schema declaration, conversion, malformed input,
idempotence, original creation times, and expiry metadata across the v1.6.0
port implementation.
