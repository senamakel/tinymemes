# Typed state module foundation

This independent slice adds a minimal tinymemes-bus crate and a compiled
TinyBus module. Existing Intent, Tier, learned records and JSON snapshot
shapes move mechanically into the contract; the library reexports them.
The contract has no inference, research, HTTP client or storage implementation.

Apply accepts an explicit content snapshot, its observed revision and a typed
operation: slang usage, meme usage, meme removal or marking a reply remixed.
The owner invokes its existing index operations and returns the resulting
snapshot. Revision comparisons are comparisons of caller-provided DATA.
They are neither durable CAS nor proof of admission, profile ownership,
verified research, source ownership, generation or a grant. The stateless
module stores nothing and never reads configuration, workspace files or URLs.

Tests first pin missing behavior, existing-library parity, independent input
snapshots, conflicts, duplicate remix markers and overflow refusal. A compiled
module test uses the real TinyBus ABI and typed proxy; no direct host adapter
may substitute this reducer for the eventual owning storage implementation.

Subsequent owning work must add captured admitted native storage and durable
conditional publication, fresh cross-process reads, research merge authority,
retired-generation refusal, joined research stop/drain and source-preserving
explicit legacy import receipts. Native module configuration may only bind
trusted host-selected authority using the canonical restricted controller
contract when available (TinyBus PR54 is currently unmerged). None of those
capabilities is represented by this content-only foundation.

## Behavioral evidence

Before implementation, the focused content suite executed eight tests:
one classic JSON compatibility control passed and seven operations failed on
the deliberate Unimplemented stub. The module suite then executed two tests:
malformed-call refusal passed and the real linked ABI/proxy operation failed
on the same stub. No compiler failure is counted as behavior evidence.

After implementation, the initial fresh all-feature workspace tests passed:
54 library unit tests (including the eight content regressions), 26 engine
integration tests, two module tests and one doctest. Final instrumented
workspace coverage also executed two added bus contract tests: 84 tests
passed, with no ignored cases. The module uses existing index
operations; no-op values retain their revision, changed values increment it,
and exhausted counters/revisions refuse. Existing snapshot JSON shapes and
library type paths are retained by reexports.

Strict workspace/all-target Clippy and a separate module build passed.
The minimal-client example loaded and executed the actual cdylib, proving
isolated values and typed comparison refusal. The strengthened separately loaded cdylib example passed all four operations,
independent content values, revision conflicts and counter exhaustion. Fresh
final default-disabled and all-feature workspace suites each passed 85 tests
(54 library + 26 engine + 2 bus + 2 ABI + 1 doctest), with no ignored cases.
Strict workspace/all-target Clippy and the rebuilt module also passed. Strict workspace
rustdoc passed after correcting an existing ambiguous module link. Final
coverage was 84.26% across the workspace: the new reducer and extracted
Intent/Tier contracts were 100%, the typed client 90.91%, and the module
94.44%. Several existing production files remain below 90%; this is not an
all-files coverage claim. Evidence is in target/gates/coverage-final.log and
target/gates/docs-final.log. Ordinary tests, Clippy, build and actual dynamic
execution logs are target/gates/default-final.log, all-features-final.log,
clippy-final.log, module-build-final.log and compiled-example-final.log. This dynamic-loader proof does not imply native
persistence or actor admission.

Normal dependency inspection (`cargo tree -p tinymemes-bus --edges normal`)
confirms the contract excludes tinymemes implementation, tinymemes-module,
tinybus-module, tinyinference and reqwest. Its loader example uses TinyBus's
module feature only as a dev-dependency. This is a composition check for the
contract; no dependency reduction or cloud runtime acceptance is claimed.
