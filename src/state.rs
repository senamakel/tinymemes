//! Stateless content operations shared by the compiled module.
use crate::{MemeIndex, SlangIndex};
use tinymemes_bus::{ApplyRequest, ApplyResult, ContentError, ContentOperation};

/// Apply an operation to caller-supplied content. No storage is accessed.
///
/// A revision comparison validates only this content value, never a store CAS
/// or grant. Usage follows the existing index semantics; exhausted counters or
/// revisions return a typed refusal instead of wrapping or panicking.
pub fn apply(request: ApplyRequest) -> ApplyResult {
    let ApplyRequest {
        mut state,
        expected_revision,
        operation,
    } = request;
    if state.revision != expected_revision {
        return Err(ContentError::RevisionConflict {
            expected: expected_revision,
            actual: state.revision,
        });
    }
    let changed = match operation {
        ContentOperation::RecordSlangUsage { region, text } => {
            let index = SlangIndex::from_snapshot(state.slang.clone());
            let terms = index.terms_in(&region, &text);
            if state.slang.terms.iter().any(|term| {
                term.region == region && terms.contains(&term.term) && term.used == u32::MAX
            }) {
                return Err(ContentError::CounterExhausted);
            }
            let used = index.record_used(&region, &text);
            state.slang = index.snapshot();
            !used.is_empty()
        }
        ContentOperation::RecordMemeUsage { url } => {
            let Some(meme) = state.memes.memes.iter().find(|meme| meme.url == url) else {
                return Ok(state);
            };
            if meme.used == u32::MAX {
                return Err(ContentError::CounterExhausted);
            }
            let index = MemeIndex::from_snapshot(state.memes);
            index.record_used(&url);
            state.memes = index.snapshot();
            true
        }
        ContentOperation::RemoveMeme { region, title } => {
            let index = MemeIndex::from_snapshot(state.memes);
            let removed = index.remove(&region, &title);
            state.memes = index.snapshot();
            removed
        }
        ContentOperation::MarkRemixed { reply_id } => {
            if state.remixed.contains(&reply_id) {
                return Ok(state);
            }
            state.remixed.push(reply_id);
            true
        }
    };
    if changed {
        state.revision = state
            .revision
            .checked_add(1)
            .ok_or(ContentError::RevisionExhausted)?;
    }
    Ok(state)
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
