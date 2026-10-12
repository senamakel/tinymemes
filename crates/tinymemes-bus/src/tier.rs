//! Remix intensity, shared without inference policy.
use serde::{Deserialize, Serialize};

/// How hard the reply gets remixed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Leave the reply alone.
    Off,
    /// A light sprinkle of slang, no images.
    Light,
    /// Casual slang and one meme.
    Spicy,
    /// Full internet voice and up to two memes.
    Unhinged,
}

impl Tier {
    /// Whether this tier runs the agent at all.
    pub fn remixes(self) -> bool {
        self != Tier::Off
    }

    /// Intensity instruction handed to the slang agent. The words themselves
    /// come from the region's lexicon.
    pub fn style(self) -> &'static str {
        match self {
            Tier::Off => "Do not change the reply.",
            Tier::Light => {
                "Keep the reply mostly as written; swap in two or three casual words. Stay clear and readable."
            }
            Tier::Spicy => {
                "Rewrite in a casual, slangy voice. Keep it punchy and fun but every point must still land."
            }
            Tier::Unhinged => {
                "Go full group-chat: heavy slang, playful exaggeration, a bit of roasting if the user is roasting. \
                 Still never drop or change a fact, step, number, or link."
            }
        }
    }
}

#[cfg(test)]
#[path = "tier_tests.rs"]
mod tests;
