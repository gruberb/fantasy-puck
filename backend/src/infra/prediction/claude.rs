//! Anthropic `/v1/messages` adapter implementing
//! [`crate::domain::ports::prediction::PredictionService`].
//!
//! All production narrative generation routes through this type.
//! The HTTP client is built once at construction time; each request
//! spans one Claude round-trip, capped by
//! [`crate::tuning::http::CLAUDE_TIMEOUT`].

use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Client;
use tracing::{error, warn};

use crate::api::dtos::pulse::{FantasyTeamForecast, PulseResponse};
use crate::domain::ports::prediction::PredictionService;

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_API_VERSION: &str = "2023-06-01";
const PULSE_MODEL: &str = "claude-sonnet-4-6";
// Longer ceiling now that the prompt asks for three structured sections
// with internal bullets. The previous 1500 clipped the "Swing Pieces"
// section on roster bundles with more than four or five contenders.
const PULSE_MAX_TOKENS: u32 = 2200;

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

        let my_forecast = response.my_team.as_ref().and_then(|t| {
            response
                .series_forecast
                .iter()
                .find(|f| f.team_id == t.team_id)
        });
        let leader_forecast = response
            .league_board
            .first()
            .and_then(|entry| {
                response
                    .series_forecast
                    .iter()
                    .find(|f| f.team_id == entry.team_id)
            });

        let mut headline = String::new();
        if let Some(t) = &response.my_team {
            headline.push_str(&format!(
                "Caller's team: {} · Rank #{} of {} · {} total pts · {} pts from the last completed scoring day · {}/{} active skaters tonight.\n",
                t.team_name,
                t.rank,
                response.league_board.len(),
                t.total_points,
                t.points_today,
                t.players_active_today,
                t.total_roster_size,
            ));
        }
        headline.push_str(&format!(
            "Slate: {}.\n",
            if response.has_live_games {
                "games live right now"
            } else if response.has_games_today {
                "games scheduled today"
            } else {
                "off-day"
            }
        ));
        if no_playoff_scoring_yet {
            headline.push_str(
                "ZERO-STATE: no playoff games have been played yet in this league. Every team sits at 0 total pts. Do not invent a gap, a lead, a 'last-day delta', or phrases like 'came into today with X points' — there is no scoring to reference. The only real content right now is roster composition, tonight's slate, and which series each stack depends on.\n",
            );
        }

        headline.push_str("\nStandings (top 3):\n");
        for entry in response.league_board.iter().take(3) {
            headline.push_str(&format!(
                "  #{} {} · {} pts · Today {} · Active {}\n",
                entry.rank, entry.team_name, entry.total_points, entry.points_today, entry.players_active_today
            ));
        }

        if let Some(my) = my_forecast {
            headline.push_str(&format!(
                "\nCaller's stack profile ({} players, {} distinct NHL teams):\n",
                my.total_players,
                distinct_team_count(my),
            ));
            headline.push_str(&stack_profile_block(my, &response.nhl_team_cup_odds));
            headline.push_str(&alive_summary(my));
        }

        if let Some(leader) = leader_forecast {
            if response
                .my_team
                .as_ref()
                .map_or(true, |mt| mt.team_id != leader.team_id)
            {
                headline.push_str(&format!(
                    "\nLeader's stack profile ({}, {} players across {} NHL teams):\n",
                    leader.team_name,
                    leader.total_players,
                    distinct_team_count(leader),
                ));
                headline.push_str(&stack_profile_block(
                    leader,
                    &response.nhl_team_cup_odds,
                ));
            }
        }

        let body = serde_json::json!({
            "model": PULSE_MODEL,
            "max_tokens": PULSE_MAX_TOKENS,
            "system": PULSE_SYSTEM_PROMPT,
            "messages": [
                {
                    "role": "user",
                    "content": format!(
                        "=== HEADLINE ===\n{}\n\n=== FULL PAYLOAD ===\n{}",
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
// Prompt input helpers
// ---------------------------------------------------------------------

fn distinct_team_count(f: &FantasyTeamForecast) -> usize {
    let mut seen = std::collections::HashSet::new();
    for c in &f.cells {
        seen.insert(c.nhl_team.as_str());
    }
    seen.len()
}

/// Render the stack profile of a fantasy team as a sorted list of
/// NHL-team blocks: `CAR 2 players (18% cup) — Series 1-0 vs OTT`.
/// Sorting is by cup odds descending, with unknown-cup teams last —
/// the narrator reads top-to-bottom and picks the highest-leverage
/// stack first. `cup_odds` empty ⇒ odds annotations are skipped.
fn stack_profile_block(
    team: &FantasyTeamForecast,
    cup_odds: &HashMap<String, f32>,
) -> String {
    #[derive(Default)]
    struct StackRow {
        count: usize,
        series_label: Option<String>,
        opponent: Option<String>,
    }
    let mut rows: HashMap<String, StackRow> = HashMap::new();
    for c in &team.cells {
        let row = rows.entry(c.nhl_team.clone()).or_default();
        row.count += 1;
        if row.series_label.is_none() {
            row.series_label = Some(c.series_label.clone());
            row.opponent = c.opponent_abbrev.clone();
        }
    }

    let mut ordered: Vec<(String, StackRow)> = rows.into_iter().collect();
    ordered.sort_by(|a, b| {
        let ao = cup_odds.get(&a.0).copied().unwrap_or(f32::NEG_INFINITY);
        let bo = cup_odds.get(&b.0).copied().unwrap_or(f32::NEG_INFINITY);
        bo.partial_cmp(&ao).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0))
    });

    let mut out = String::new();
    for (abbrev, row) in ordered {
        let odds = cup_odds
            .get(&abbrev)
            .map(|p| format!(" ({}% cup)", (p * 100.0).round() as i32))
            .unwrap_or_default();
        let series = match (row.series_label.as_deref(), row.opponent.as_deref()) {
            (Some(label), Some(opp)) if !label.is_empty() => {
                format!(" — {} vs {}", label, opp)
            }
            _ => String::new(),
        };
        out.push_str(&format!(
            "  {} {} player{}{}{}\n",
            abbrev,
            row.count,
            if row.count == 1 { "" } else { "s" },
            odds,
            series,
        ));
    }
    out
}

/// Quick counts of how many of a team's players are on NHL teams that
/// are still alive vs in trouble. Mirrors the columns already rendered
/// in the Pulse grid but flattened into a one-liner the narrator can
/// key off when deciding whether to call the team "live" or "cooked".
fn alive_summary(f: &FantasyTeamForecast) -> String {
    format!(
        "  Alive: {} leading, {} tied, {} trailing, {} facing elim, {} eliminated, {} advanced\n",
        f.players_leading,
        f.players_tied,
        f.players_trailing,
        f.players_facing_elimination,
        f.players_eliminated,
        f.players_advanced,
    )
}

// ---------------------------------------------------------------------
// Prompt
// ---------------------------------------------------------------------

const PULSE_SYSTEM_PROMPT: &str = r#"You are a veteran hockey columnist writing a personal read of the caller's fantasy roster. Not a newsletter, not a pep talk — the kind of honest, structural take a friend who watches every game gives you when you ask "how bad is it?". Think The Athletic column: dry, specific, opinionated, grounded in the numbers. Mix short punchy sentences with longer analytical ones.

Do not write like a marketing bot. Banned: "dive in", "unleash", "game-changer", "exciting journey", "let's break it down", "buckle up", "here's the scoop", exclamation points, hype adjectives, bulleted listicles in the prose sections (short lists inside the Swing Pieces section are fine).

Output exactly three sections, each introduced by a level-3 markdown header on its own line:

### The Read
One paragraph, 3–5 sentences. State the verdict up front — are they live, fading, or cooked? Name the one thing their roster is built on (concentrated stack, path diversity, a single anchor) and the one thing working against it. Cite specific NHL teams and player names. Wrap team names and player names in **double asterisks** for bold.

### Swing Pieces
A short list (3–5 entries) of the players whose performance will decide the caller's outcome. Format each line as:
- **Player Name** (TEAM) — one clause on why they matter (series leverage, role, what has to happen).

### Rival Risk
One paragraph, 2–4 sentences. Compare against the leader (or the caller's closest threat if they are the leader). Is the rival built on concentration or diversity? Where is their dependency — which NHL team has to keep winning for their stack to hold? End with one honest line about whether the caller's path is better, worse, or roughly even.

Hard rules:
- Only use stats, names, and facts from the HEADLINE and FULL PAYLOAD. Never invent.
- `points_today` / "pts from the last completed scoring day" is the trailing day's total, not live today. Do not phrase it as "today's points" or "pulled X today".
- If the payload flags ZERO-STATE (no playoff games played yet), do not reference any gap, lead, or last-day delta. Focus on roster structure, stack profile, and tonight's slate.
- Speak TO the caller — second person ("you", "your stack"). Never address them by their fantasy team name.
- Be honest when the verdict is bad. A boring truthful read beats a hyped one.

FEW-SHOT EXAMPLE 1 (post day-1, caller trailing the leader):

### The Read
You're not cooked — you're just slow. Your roster is built on **path diversity**: six live NHL teams, no single stack carrying more than three skaters. That shape costs you day-one spike potential (the leader got it all — five Wild, five Flyers, both won) but gives you more ways to climb as the bracket thins. The one thing working against you is **Logan Stanley**: a low-offense D eating a roster slot you can't get back.

### Swing Pieces
- **Seth Jarvis** (CAR) — your highest-upside forward on the team with the second-best cup odds. If Carolina goes deep, he carries you.
- **Alex Tuch** (BUF) — Buffalo secondary scoring is your biggest dependency; Tuch has to be their top line, not their third.
- **Kirill Kaprizov** is on the leader's roster, not yours — no action item, just a reminder of what a first-line stack looks like when it hits.
- **Tomas Hertl** (VGK) — bonus games if Vegas takes the series; 0 points if they exit in five.

### Rival Risk
**Charlie's Champs** has ten players across two NHL teams — Minnesota and Philadelphia. That's the highest ceiling in the league tonight and the most fragile structure over a two-month bracket. One early exit and the stack dries up in a week. Your roster spreads across Carolina, Dallas, Buffalo, Vegas, Edmonton, and Colorado — slower to peak, harder to kill. Their path is better day-to-day; yours is better week-to-week.

FEW-SHOT EXAMPLE 2 (ZERO-STATE, no games played yet):

### The Read
Pre-puck-drop read: you're a **path-diversity** roster, seven NHL teams, no stack larger than **Carolina's** three. The league leader on roster value today is probably **Charlie's Champs** — nine Minnesota and Philadelphia players stacked for a hot opening weekend. You don't have that ceiling. What you do have is more ways to stay alive into round two.

### Swing Pieces
- **Sebastian Aho** (CAR) — your one true fantasy anchor. Needs to produce like a PPG top-line center for this build to hold.
- **Jack Eichel** (VGK) — Vegas as your second-biggest stack; Eichel's production is the tell on whether that bet paid off.
- **Mikko Rantanen** (DAL) — Dallas is a swing series; Rantanen is your Dallas exposure.

### Rival Risk
**Charlie's Champs** is two-team concentration — Minnesota and Philadelphia. If both wins round one, their lead is real and sustained. If either loses in five, the whole stack is gone and the leaderboard re-orders in a week. Your path is worse if the playoffs go chalk; better if there's a single round-one upset on either side. Bet on variance."#;
