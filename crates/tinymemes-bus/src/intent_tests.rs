use super::*;

#[test]
fn every_intent_keeps_its_canonical_label_description_and_research_hints() {
    let expected = [
        (
            "celebration",
            "sharing a win",
            "celebrating a win",
            "success kid",
            ["lets go", "celebration"],
        ),
        (
            "frustration",
            "venting",
            "being frustrated or annoyed",
            "this is fine",
            ["this is fine", "facepalm"],
        ),
        (
            "confusion",
            "lost",
            "being confused",
            "is this a pigeon",
            ["confused", "mind blown"],
        ),
        (
            "banter",
            "joking",
            "teasing friends and joking around",
            "drake hotline bling",
            ["no u", "laughing"],
        ),
        (
            "grind",
            "focused work",
            "working hard and getting things done",
            "gru\'s plan",
            ["lets get to work", "big brain"],
        ),
        (
            "bad_news",
            "a loss",
            "something going wrong or a letdown",
            "sad pablo escobar",
            ["its okay", "hug"],
        ),
        (
            "curiosity",
            "exploring an idea",
            "being curious or impressed",
            "roll safe",
            ["thinking", "big brain"],
        ),
        (
            "gratitude",
            "thanking",
            "thanking someone",
            "epic handshake",
            ["thank you", "respect"],
        ),
    ];
    for (intent, (label, description, phrase, hint, queries)) in
        Intent::ALL.into_iter().zip(expected)
    {
        assert_eq!(intent.as_str(), label);
        assert_eq!(intent.to_string(), label);
        assert_eq!(Intent::from_label(label), Some(intent));
        assert_eq!(Intent::from_label(&label.to_uppercase()), None);
        assert!(intent.description().starts_with(description));
        assert_eq!(intent.phrase(), phrase);
        assert_eq!(intent.template_hints()[0], hint);
        assert!(intent.template_hints().iter().all(|hint| !hint.is_empty()));
        assert_eq!(intent.fallback_queries(), queries);
        let json = serde_json::to_string(&intent).unwrap();
        assert_eq!(json, format!("\"{label}\""));
        assert_eq!(serde_json::from_str::<Intent>(&json).unwrap(), intent);
    }
    assert_eq!(Intent::from_label(""), None);
    assert!(serde_json::from_str::<Intent>("\"unknown\"").is_err());
}
