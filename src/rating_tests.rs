use super::*;

fn reading(frankness: f64, playful: f64, serious: f64, intent: Intent) -> Reading {
    Reading {
        chat_intent: intent,
        reply_intent: intent,
        reply_intent_confidence: 0.9,
        frankness,
        playful,
        serious,
        slang_best: None,
        slang_enough: None,
        user_language: None,
        meme: crate::reading::MemePick::Unasked,
        reply_matches_user: None,
        meme_p: None,
        meme_weak: None,
    }
}

#[test]
fn serious_chat_is_off_however_frank() {
    let r = RatingPolicy::default().rate(&reading(1.0, 1.0, 0.8, Intent::Banter));
    assert_eq!((r.score, r.tier, r.max_memes), (0, Tier::Off, 0));
}

#[test]
fn frank_playful_banter_is_unhinged() {
    let r = RatingPolicy::default().rate(&reading(0.9, 0.9, 0.05, Intent::Banter));
    assert_eq!(r.tier, Tier::Unhinged);
    assert_eq!(r.max_memes, 1);
}

#[test]
fn formal_chat_stays_off() {
    let r = RatingPolicy::default().rate(&reading(0.1, 0.2, 0.0, Intent::Grind));
    assert_eq!(r.tier, Tier::Off);
}

#[test]
fn bad_news_is_capped_at_light() {
    let r = RatingPolicy::default().rate(&reading(1.0, 1.0, 0.2, Intent::BadNews));
    assert_eq!(r.tier, Tier::Light);
    assert_eq!(r.max_memes, 0);
}

#[test]
fn middling_chat_is_spicy() {
    let r = RatingPolicy::default().rate(&reading(0.6, 0.5, 0.1, Intent::Curiosity));
    assert_eq!(r.tier, Tier::Spicy);
}

fn picked(mut r: Reading, p: f64) -> Reading {
    r.meme = crate::reading::MemePick::Pick("Jethalal Thinking".into());
    r.meme_p = Some(p);
    r
}

#[test]
fn a_confident_pick_lifts_light_and_spicy_to_one_meme() {
    let policy = RatingPolicy::default();
    // Light: frank enough to remix, not enough for a meme on its own.
    let light = picked(reading(0.4, 0.3, 0.05, Intent::Curiosity), 0.75);
    let rating = policy.rate(&light);
    assert_eq!((rating.tier, rating.max_memes), (Tier::Light, 0));
    assert!(policy.strong_pick_allows(&rating, &light));

    let spicy = picked(reading(0.6, 0.6, 0.05, Intent::Banter), 0.7);
    let rating = policy.rate(&spicy);
    assert_eq!(rating.tier, Tier::Spicy);
    assert!(policy.strong_pick_allows(&rating, &spicy));
}

#[test]
fn a_weak_pick_or_no_pick_earns_nothing() {
    let policy = RatingPolicy::default();
    let weak = picked(reading(0.4, 0.3, 0.05, Intent::Curiosity), 0.69);
    assert!(!policy.strong_pick_allows(&policy.rate(&weak), &weak));
    let mut none = reading(0.4, 0.3, 0.05, Intent::Curiosity);
    none.meme = crate::reading::MemePick::NoneFit;
    none.meme_p = Some(0.9);
    assert!(!policy.strong_pick_allows(&policy.rate(&none), &none));
}

#[test]
fn a_strong_pick_never_reaches_off_unhinged_or_bad_news() {
    let policy = RatingPolicy::default();
    let off = picked(reading(0.1, 0.1, 0.0, Intent::Grind), 0.95);
    assert!(!policy.strong_pick_allows(&policy.rate(&off), &off));
    // Unhinged already allows its own meme; nothing to lift.
    let unhinged = picked(reading(0.9, 0.9, 0.05, Intent::Banter), 0.95);
    assert!(!policy.strong_pick_allows(&policy.rate(&unhinged), &unhinged));
    let bad = picked(reading(1.0, 1.0, 0.2, Intent::BadNews), 0.95);
    let rating = policy.rate(&bad);
    assert_eq!(rating.tier, Tier::Light);
    assert!(!policy.strong_pick_allows(&rating, &bad));
}
