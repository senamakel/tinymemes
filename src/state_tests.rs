use super::*;
use crate::{IndexPolicy, Region, SlangIndex};
use tinymemes_bus::{ContentOperation, ContentState};

fn request(state: ContentState, operation: ContentOperation) -> ApplyRequest {
    ApplyRequest {
        expected_revision: state.revision,
        state,
        operation,
    }
}
fn seeded() -> ContentState {
    let index = SlangIndex::new(IndexPolicy::default());
    index.seed(&Region::india());
    ContentState {
        slang: index.snapshot(),
        ..Default::default()
    }
}
#[test]
fn slang_usage_matches_existing_index_and_preserves_other_content() {
    let input = seeded();
    let classic = SlangIndex::from_snapshot(input.slang.clone());
    classic.record_used("IN", "yaar yaar ekdum sorted");
    let output = apply(request(
        input.clone(),
        ContentOperation::RecordSlangUsage {
            region: "IN".into(),
            text: "yaar yaar ekdum sorted".into(),
        },
    ))
    .unwrap();
    assert_eq!(output.slang, classic.snapshot());
    assert_eq!(output.memes, input.memes);
    assert_eq!(output.remixed, input.remixed);
    assert_eq!(output.revision, 1);
    assert_eq!(input.revision, 0);
}
#[test]
fn conflicting_revision_refuses_before_content_transformation() {
    let mut req = request(
        ContentState::default(),
        ContentOperation::MarkRemixed {
            reply_id: "reply".into(),
        },
    );
    req.expected_revision = 7;
    assert_eq!(
        apply(req),
        Err(ContentError::RevisionConflict {
            expected: 7,
            actual: 0
        })
    );
}
#[test]
fn remix_markers_are_idempotent_and_independent_inputs_do_not_share_state() {
    let state = apply(request(
        ContentState::default(),
        ContentOperation::MarkRemixed {
            reply_id: "reply".into(),
        },
    ))
    .unwrap();
    assert_eq!(state.remixed, ["reply"]);
    let same = apply(request(
        state.clone(),
        ContentOperation::MarkRemixed {
            reply_id: "reply".into(),
        },
    ))
    .unwrap();
    assert_eq!(same, state);
    let other = apply(request(
        ContentState::default(),
        ContentOperation::MarkRemixed {
            reply_id: "other".into(),
        },
    ))
    .unwrap();
    assert_eq!(other.remixed, ["other"]);
}
#[test]
fn revision_exhaustion_is_a_typed_refusal_but_noop_at_max_is_allowed() {
    let state = ContentState {
        revision: u64::MAX,
        remixed: vec!["reply".into()],
        ..Default::default()
    };
    assert_eq!(
        apply(request(
            state.clone(),
            ContentOperation::MarkRemixed {
                reply_id: "reply".into()
            }
        ))
        .unwrap(),
        state
    );
    assert_eq!(
        apply(request(
            state,
            ContentOperation::MarkRemixed {
                reply_id: "new".into()
            }
        )),
        Err(ContentError::RevisionExhausted)
    );
}

fn meme(url: &str, title: &str, region: &str) -> tinymemes_bus::LearnedMeme {
    tinymemes_bus::LearnedMeme {
        region: region.into(),
        title: title.into(),
        meaning: "meaning".into(),
        url: url.into(),
        page_url: "https://example.org/page".into(),
        intents: vec![crate::Intent::Banter],
        tags: vec![],
        rating: "g".into(),
        verified: 0.9,
        used: 0,
        added_at: 7,
    }
}
#[test]
fn meme_usage_and_exact_removal_match_existing_index() {
    let state = ContentState {
        memes: tinymemes_bus::MemeSnapshot {
            memes: vec![
                meme("url", "Same", "IN"),
                meme("url", "Same", "US"),
                meme("other", "same", "IN"),
            ],
            searches: vec![],
        },
        ..Default::default()
    };
    let classic = crate::MemeIndex::from_snapshot(state.memes.clone());
    classic.record_used("url");
    let used = apply(request(
        state,
        ContentOperation::RecordMemeUsage { url: "url".into() },
    ))
    .unwrap();
    assert_eq!(used.memes, classic.snapshot());
    assert_eq!(used.memes.memes[0].used, 1);
    assert_eq!(used.memes.memes[1].used, 0);
    classic.remove("IN", "Same");
    let removed = apply(request(
        used,
        ContentOperation::RemoveMeme {
            region: "IN".into(),
            title: "Same".into(),
        },
    ))
    .unwrap();
    assert_eq!(removed.memes, classic.snapshot());
    assert_eq!(removed.revision, 2);
}
#[test]
fn usage_exhaustion_refuses_instead_of_panicking_or_wrapping() {
    let mut state = seeded();
    let term = state.slang.terms.first_mut().unwrap();
    term.used = u32::MAX;
    let text = term.term.clone();
    assert_eq!(
        apply(request(
            state,
            ContentOperation::RecordSlangUsage {
                region: "IN".into(),
                text
            }
        )),
        Err(ContentError::CounterExhausted)
    );
    let mut row = meme("url", "title", "IN");
    row.used = u32::MAX;
    let state = ContentState {
        memes: tinymemes_bus::MemeSnapshot {
            memes: vec![row],
            searches: vec![],
        },
        ..Default::default()
    };
    assert_eq!(
        apply(request(
            state,
            ContentOperation::RecordMemeUsage { url: "url".into() }
        )),
        Err(ContentError::CounterExhausted)
    );
}
#[test]
fn absent_usage_and_removal_are_revision_preserving_noops() {
    let state = seeded();
    for operation in [
        ContentOperation::RecordSlangUsage {
            region: "other".into(),
            text: "yaar".into(),
        },
        ContentOperation::RecordMemeUsage {
            url: "unknown".into(),
        },
        ContentOperation::RemoveMeme {
            region: "IN".into(),
            title: "unknown".into(),
        },
    ] {
        assert_eq!(apply(request(state.clone(), operation)).unwrap(), state);
    }
}
#[test]
fn extracted_snapshot_json_remains_compatible_with_classic_restoration() {
    let state = seeded();
    let json = serde_json::to_string(&state.slang).unwrap();
    assert_eq!(
        SlangIndex::from_json(&json, IndexPolicy::default())
            .unwrap()
            .snapshot(),
        state.slang
    );
    let memes = tinymemes_bus::MemeSnapshot {
        memes: vec![meme("url", "title", "IN")],
        searches: vec![],
    };
    let json = serde_json::to_string(&memes).unwrap();
    assert_eq!(
        crate::MemeIndex::from_json(&json, crate::MemeIndexPolicy::default())
            .unwrap()
            .snapshot(),
        memes
    );
}
