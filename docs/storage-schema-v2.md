# Storage on the v2 schema

`tinymemes-module` (feature `storage-schema`, off by default so the loadable
module does not link the storage core) exports
`storage_schema() -> tinystoragedrivers_core::Schema`. A host composes it with
`Registry::compose`, and the module's storage grant is generated from its items.

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
