use super::*;
use std::collections::HashMap;

fn cfg(pairs: &[(&str, &str)]) -> EnvConfig {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    EnvConfig::from_lookup(|k| map.get(k).cloned())
}

#[test]
fn unset_env_defers_everything_to_the_host() {
    let c = cfg(&[]);
    assert!(c.openrouter_key.is_none());
    assert_eq!(c.model_id(), DEFAULT_MODEL);
    assert!(c.jev().unwrap().is_none());
    assert!(c.chat_model(&reqwest::Client::new()).is_none());
}

#[test]
fn blank_values_count_as_unset() {
    let c = cfg(&[("TINYMEMES_OPENROUTER_KEY", "  "), ("TINYMEMES_MODEL", "")]);
    assert!(c.openrouter_key.is_none());
    assert_eq!(c.model_id(), DEFAULT_MODEL);
}

#[test]
fn explicit_modes_pick_routes_or_fail_loudly() {
    let c = cfg(&[("TINYMEMES_JEV", "llm"), ("TYPESAFE_API_KEY", "ts")]);
    assert!(c.forces_llm_jev());
    assert!(c.jev().unwrap().is_none());

    let c = cfg(&[("TINYMEMES_JEV", "typesafe"), ("TYPESAFE_API_KEY", "ts")]);
    assert_eq!(c.jev().unwrap().unwrap().1, "typesafe");

    let c = cfg(&[("TINYMEMES_JEV", "openrouter")]);
    assert!(c.jev().is_err());

    // Unset mode: an env OpenRouter key alone does not claim Jev (the host's
    // managed Jev comes first); explicit `auto` lets it.
    let c = cfg(&[("TINYMEMES_OPENROUTER_KEY", "or")]);
    assert!(c.jev().unwrap().is_none());
    let c = cfg(&[
        ("TINYMEMES_OPENROUTER_KEY", "or"),
        ("TINYMEMES_JEV", "auto"),
    ]);
    assert_eq!(c.jev().unwrap().unwrap().1, "openrouter");
}

#[test]
fn debug_never_prints_keys() {
    let c = cfg(&[
        ("TINYMEMES_OPENROUTER_KEY", "sk-openrouter"),
        ("TINYMEMES_TYPESAFE_KEY", "sk-typesafe"),
        ("TINYMEMES_OPENJEV_KEY", "sk-openjev"),
    ]);
    let debug = format!("{c:?}");
    for secret in ["sk-openrouter", "sk-typesafe", "sk-openjev"] {
        assert!(!debug.contains(secret));
    }
    assert!(debug.contains("<set>"));
}

#[test]
fn mode_spellings_and_standard_key_fallbacks_are_supported() {
    for (mode, expected) in [
        ("OpenJev", JevMode::OpenJev),
        ("open_jev", JevMode::OpenJev),
        ("off", JevMode::Llm),
        ("none", JevMode::Llm),
        ("future-mode", JevMode::Auto),
    ] {
        let c = cfg(&[("TINYMEMES_JEV", mode)]);
        assert_eq!(c.jev, Some(expected), "mode {mode:?}");
    }
    let c = cfg(&[
        ("TYPESAFE_API_KEY", " fallback-ts "),
        ("OPENJEV_API_KEY", "fallback-jev"),
    ]);
    assert_eq!(c.typesafe_key.as_deref(), Some("fallback-ts"));
    assert_eq!(c.openjev_key.as_deref(), Some("fallback-jev"));
    assert_eq!(c.jev().unwrap().unwrap().1, "typesafe");
    assert_eq!(cfg(&[("TINYMEMES_JEV", "")]).jev, None);
}

#[test]
fn explicit_routes_require_their_own_credentials_and_llm_mode_has_no_jev() {
    for mode in ["typesafe", "openjev", "openrouter"] {
        let c = cfg(&[("TINYMEMES_JEV", mode)]);
        assert!(c.jev().is_err(), "missing credential for {mode}");
    }
    let c = cfg(&[("TINYMEMES_JEV", "llm"), ("TINYMEMES_OPENROUTER_KEY", "or")]);
    assert!(c.forces_llm_jev());
    assert!(c.jev().unwrap().is_none());
}

#[test]
fn configured_chat_and_web_clients_share_model_selection() {
    let c = cfg(&[
        ("TINYMEMES_OPENROUTER_KEY", "key"),
        ("TINYMEMES_MODEL", "mymodel"),
    ]);
    let http = reqwest::Client::new();
    assert!(c.chat_model(&http).is_some());
    assert!(c.web_researcher(&http).is_some());
}
