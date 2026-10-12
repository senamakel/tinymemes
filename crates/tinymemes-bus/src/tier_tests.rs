use super::*;

#[test]
fn tiers_keep_order_wire_labels_and_each_existing_style_instruction() {
    let cases = [
        (Tier::Off, "off", "Do not change the reply."),
        (Tier::Light, "light", "two or three"),
        (Tier::Spicy, "spicy", "punchy"),
        (Tier::Unhinged, "unhinged", "roasting"),
    ];
    for (tier, label, style) in cases {
        assert_eq!(tier.remixes(), tier != Tier::Off);
        assert!(tier.style().contains(style));
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, format!("\"{label}\""));
        assert_eq!(serde_json::from_str::<Tier>(&json).unwrap(), tier);
    }
    assert!(Tier::Off < Tier::Light && Tier::Light < Tier::Spicy && Tier::Spicy < Tier::Unhinged);
}
