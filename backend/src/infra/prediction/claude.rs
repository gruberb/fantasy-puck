//! Anthropic `/v1/messages` adapter implementing
//! [`crate::domain::ports::prediction::PredictionService`].
//!
//! All production narrative generation routes through this type.
//! The HTTP client is built once at construction time; each request
//! spans one Claude round-trip, capped by
//! [`crate::tuning::http::CLAUDE_TIMEOUT`].

use async_trait::async_trait;
use reqwest::Client;
use tracing::{error, warn};

use crate::api::dtos::pulse::PulseResponse;
use crate::domain::ports::prediction::PredictionService;

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_API_VERSION: &str = "2023-06-01";
const PULSE_MODEL: &str = "claude-sonnet-4-6";
const PULSE_MAX_TOKENS: u32 = 1500;

pub struct ClaudeNarrator {
    api_key: String,
    http: Client,
}

impl ClaudeNarrator {
    /// Build a `ClaudeNarrator` from the `ANTHROPIC_API_KEY` env var.
    /// Returns `None` if the key is unset — in that case the main
    /// composition root falls back to a [`NullNarrator`] so the rest
    /// of the server still boots.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok()?;
        let http = Client::builder()
            .timeout(crate::tuning::http::CLAUDE_TIMEOUT)
            .build()
            .ok()?;
        Some(Self { api_key, http })
    }
}

#[async_trait]
impl PredictionService for ClaudeNarrator {
    async fn pulse_narrative(&self, response: &PulseResponse) -> Option<String> {
        let payload = serde_json::to_string(response).ok()?;
        let no_playoff_scoring_yet = response
            .league_board
            .iter()
            .all(|e| e.total_points == 0 && e.points_today == 0);

        let mut headline = String::new();
        if let Some(t) = &response.my_team {
            headline.push_str(&format!(
                "Caller's team: {} · Rank #{} · {} total playoff pts · {} pts from the last completed scoring day · {}/{} players have an NHL game scheduled today.\n",
                t.team_name,
                t.rank,
                t.total_points,
                t.points_today,
                t.players_active_today,
                t.total_roster_size,
            ));
        }
        headline.push_str(&format!(
            "League has {} teams. {}.\n",
            response.league_board.len(),
            if response.has_live_games {
                "Games live right now"
            } else if response.has_games_today {
                "Games scheduled today"
            } else {
                "Off-day"
            }
        ));
        if no_playoff_scoring_yet {
            headline.push_str(
                "ZERO-STATE: no playoff games have been played yet in this league. Every team sits at 0 playoff points. Do not invent a gap, a lead, a 'last-day delta', or phrases like 'came into today with X points' — there is no scoring to reference. The only real content right now is who has how many active skaters tonight and which NHL matchups those skaters are in.\n",
            );
        }
        for entry in response.league_board.iter().take(3) {
            headline.push_str(&format!(
                "  #{} {} · {} pts\n",
                entry.rank, entry.team_name, entry.total_points
            ));
        }

        let body = serde_json::json!({
            "model": PULSE_MODEL,
            "max_tokens": PULSE_MAX_TOKENS,
            "system": PULSE_SYSTEM_PROMPT,
            "messages": [
                {
                    "role": "user",
                    "content": format!(
                        "=== HEADLINE NUMBERS ===\n{}\n\n=== FULL PAYLOAD ===\n{}",
                        headline, payload
                    )
                }
            ]
        });

        let http_response = match self
            .http
            .post(CLAUDE_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", CLAUDE_API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("pulse_narrative: Claude API call failed: {}", e);
                return None;
            }
        };

        if !http_response.status().is_success() {
            let status = http_response.status();
            let body = http_response.text().await.unwrap_or_default();
            warn!("pulse_narrative: Claude returned {}: {}", status, body);
            return None;
        }

        let body: serde_json::Value = match http_response.json().await {
            Ok(v) => v,
            Err(e) => {
                error!("pulse_narrative: failed to parse Claude response: {}", e);
                return None;
            }
        };
        body.get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

/// Fallback implementation used when `ANTHROPIC_API_KEY` is unset
/// (local dev without Anthropic credentials). Every narrative call
/// returns `None` so the Pulse / Insights pages render without the
/// narrative block rather than refusing to start the server.
pub struct NullNarrator;

#[async_trait]
impl PredictionService for NullNarrator {
    async fn pulse_narrative(&self, _: &PulseResponse) -> Option<String> {
        None
    }
}

// ---------------------------------------------------------------------
// Prompt
// ---------------------------------------------------------------------

const PULSE_SYSTEM_PROMPT: &str = r#"You are a veteran hockey columnist writing one personal dispatch for a friend in their fantasy league. Not a newsletter, not a pep talk — a direct read of where they stand and what matters. Think The Athletic beat column: dry, specific, opinionated, grounded in the numbers. Mix short punchy sentences with longer analytical ones.

Do not write like a marketing bot. Banned phrases and styles: "dive in", "unleash", "game-changer", "exciting journey", "let's break it down", "buckle up", "here's the scoop", bulleted listicles, exclamation points, hype adjectives. No section headers.

Rules:
- Only reference stats, names, records, and facts from the data provided.
- Never invent numbers.
- Wrap player names and fantasy-team names in **double asterisks** for bold.
- 4–7 sentences. Start on the verdict, not the weather.
- `points_today` / "pts from the last completed scoring day" is yesterday's daily total (or the last day whose games were processed), NOT live scoring from games happening right now. If today is day 1 of a new round, treat those numbers as the trailing day's work, never as "today's points". Phrases like "pulling X today" or "generating X off Y active players today" are wrong — say "came into today with X" or "closed the last day with X".

The frame: speak TO the caller (second person — "you", "your team"). This is their Pulse page, not a broadcast. Anchor on their rank, their gap to first, their closest threat, what today's slate means for them specifically, and any obvious read on which of their rostered NHL teams is carrying them. Be honest if the verdict isn't good."#;
