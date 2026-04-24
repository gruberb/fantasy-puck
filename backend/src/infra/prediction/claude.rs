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

use crate::domain::ports::prediction::PredictionService;

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_API_VERSION: &str = "2023-06-01";
const MODEL: &str = "claude-sonnet-4-6";
// Large enough for four markdown sections with bullet lists in the
// Player-by-Player block — a 10-skater roster can run to ~10 bullets.
const MAX_TOKENS: u32 = 2600;

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
    async fn team_diagnosis(
        &self,
        team: &crate::api::dtos::teams::TeamPointsResponse,
    ) -> Option<String> {
        let Some(diagnosis) = team.diagnosis.as_ref() else {
            return None;
        };
        let payload = serde_json::to_string(team).ok()?;
        let headline = build_team_diagnosis_headline(team, diagnosis);

        let body = serde_json::json!({
            "model": MODEL,
            "max_tokens": MAX_TOKENS,
            "system": TEAM_DIAGNOSIS_SYSTEM_PROMPT,
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
                warn!("team_diagnosis: Claude API call failed: {}", e);
                return None;
            }
        };

        if !http_response.status().is_success() {
            let status = http_response.status();
            let body = http_response.text().await.unwrap_or_default();
            warn!("team_diagnosis: Claude returned {}: {}", status, body);
            return None;
        }

        let body: serde_json::Value = match http_response.json().await {
            Ok(v) => v,
            Err(e) => {
                error!("team_diagnosis: failed to parse Claude response: {}", e);
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
/// returns `None` so the Pulse page renders without the narrative
/// block rather than refusing to start the server.
pub struct NullNarrator;

#[async_trait]
impl PredictionService for NullNarrator {
    async fn team_diagnosis(
        &self,
        _: &crate::api::dtos::teams::TeamPointsResponse,
    ) -> Option<String> {
        None
    }
}

// ---------------------------------------------------------------------
// Prompt input + system prompt for the team-diagnosis narrative.
// ---------------------------------------------------------------------

fn build_team_diagnosis_headline(
    team: &crate::api::dtos::teams::TeamPointsResponse,
    diagnosis: &crate::api::dtos::teams::TeamDiagnosis,
) -> String {
    use crate::domain::prediction::grade::{Grade, PlayerBucket};
    let mut out = String::new();
    out.push_str(&format!(
        "Caller's team: {} · Rank #{} of {} · {} total pts · {} behind 1st · {} ahead of 3rd.\n",
        team.team_name,
        diagnosis.league_rank,
        diagnosis.league_size,
        team.team_totals.total_points,
        diagnosis.gap_to_first,
        diagnosis.gap_to_third,
    ));
    out.push_str("\nConcentration (rostered per NHL team, playoff pts from that team):\n");
    for c in diagnosis.concentration_by_team.iter().take(12) {
        out.push_str(&format!(
            "  {} · {} players · {} pts\n",
            c.nhl_team, c.rostered, c.team_playoff_points
        ));
    }
    out.push_str("\nYesterday:\n");
    let y = &diagnosis.yesterday;
    out.push_str(&format!(
        "  date {date} · NHL games {games} ({completed} completed) · caller {g}G {a}A {p}P\n",
        date = y.date,
        games = y.nhl_games,
        completed = y.completed_games,
        g = y.my_goals,
        a = y.my_assists,
        p = y.my_points,
    ));
    if y.my_players.is_empty() {
        out.push_str("  caller players: none appeared\n");
    } else {
        out.push_str("  caller players:\n");
        for p in &y.my_players {
            out.push_str(&format!(
                "    {name} ({team}) · {g}G {a}A {pts}P\n",
                name = p.name,
                team = p.nhl_team,
                g = p.goals,
                a = p.assists,
                pts = p.points,
            ));
        }
    }
    let source_label = if y.league_top_three_source == "yesterday" {
        "yesterday fantasy top 3"
    } else {
        "current playoff top 3 fallback"
    };
    out.push_str(&format!("  {source_label}:\n"));
    for (i, t) in y.league_top_three.iter().enumerate() {
        out.push_str(&format!(
            "    #{rank} {team} · {g}G {a}A {pts}P\n",
            rank = i + 1,
            team = t.team_name,
            g = t.goals,
            a = t.assists,
            pts = t.points,
        ));
    }
    out.push_str("\nRoster lines:\n");
    for p in &team.players {
        let b = match &p.breakdown {
            Some(b) => b,
            None => continue,
        };
        let grade_str = match b.grade.grade {
            Grade::A => "A",
            Grade::B => "B",
            Grade::C => "C",
            Grade::D => "D",
            Grade::F => "F",
            Grade::NotEnoughData => "–",
        };
        let bucket_str = match b.bucket {
            PlayerBucket::TooEarly => "TOO EARLY",
            PlayerBucket::Outperforming => "OUTPERFORMING",
            PlayerBucket::OnPace => "ON PACE",
            PlayerBucket::KeepFaith => "KEEP FAITH",
            PlayerBucket::FineButFragile => "FINE BUT FRAGILE",
            PlayerBucket::NeedMiracle => "NEED MIRACLE",
            PlayerBucket::ProblemAsset => "PROBLEM ASSET",
            PlayerBucket::TeamEliminated => "TEAM ELIMINATED",
        };
        out.push_str(&format!(
            "  [{bucket}] {name} ({team_} · {pos}) · {gp} GP · {g}G {a}A {pts}P · SOG {sog} · +/- {pm:+} · TOI/gm {toi_mm}:{toi_ss:02} · proj {proj:.2} PPG · grade {grade}{active} · remP {remp:.1}\n",
            bucket = bucket_str,
            name = p.name,
            team_ = p.nhl_team,
            pos = p.position,
            gp = b.games_played,
            g = p.goals,
            a = p.assists,
            pts = p.total_points,
            sog = b.sog,
            pm = b.plus_minus,
            toi_mm = b.toi_seconds_per_game / 60,
            toi_ss = (b.toi_seconds_per_game % 60).max(0),
            proj = b.projected_ppg,
            grade = grade_str,
            active = if b.active_prob < 1.0 { " (scratch risk)" } else { "" },
            remp = b.remaining_impact.expected_remaining_points,
        ));
        if !b.recent_games.is_empty() {
            let recent: Vec<String> = b
                .recent_games
                .iter()
                .map(|g| {
                    let toi = g
                        .toi_seconds
                        .map(|s| format!("{}:{:02}", s / 60, (s % 60).max(0)))
                        .unwrap_or_else(|| "--".into());
                    format!("vs {} {}P TOI {}", g.opponent, g.points, toi)
                })
                .collect();
            out.push_str(&format!("    recent: {}\n", recent.join(" | ")));
        }
    }
    out
}

const TEAM_DIAGNOSIS_SYSTEM_PROMPT: &str = r#"You are a veteran hockey columnist writing a descriptive read of the caller's fantasy roster — not advice, not action items, just "here's what happened, here's why, here's what to expect." The roster is locked for the playoffs; there are no trades, no drops, no waiver moves. The reader already knows they can't change anything. They want to understand what they drafted and what it's likely to do.

Think The Athletic column: dry, specific, grounded in the numbers. Descriptive, not prescriptive.

Do NOT write like a marketing bot. Do NOT recommend trades, drops, lineup changes, or "who to watch" — the caller isn't choosing anything. Banned: "keep faith", "need a miracle", "who to watch tonight", "drop", "pick up", "start", "sit", "who to start", any phrasing that implies a roster decision. Banned: "dive in", "unleash", "game-changer", "exciting journey", "let's break it down", "buckle up", exclamation points, hype adjectives.

Output exactly four sections, each introduced by a level-3 markdown header on its own line:

### Yesterday
One paragraph, 2–4 sentences. Explain what happened on the previous hockey date and why the caller moved or did not move: which of the caller's rostered players appeared, who scored, who was quiet, and how the top fantasy teams did that day. If there were no NHL games, say so and use the current playoff top-3 fallback to situate the league race. If NHL games happened but none of the caller's skaters appeared, say that directly and use the fantasy top-3/fallback lines rather than inventing action.

### Where You Stand
One paragraph, 3–5 sentences. State the current rank and the structural reason for it. Name the shape of the roster (concentrated vs diversified), which stacks have produced, which haven't, and whether that's a finishing-luck story or a role-and-deployment story. Cite specific NHL teams, player names, numbers. Wrap team names and player names in **double asterisks** for bold.

### Player-by-Player
A short list covering the players whose situation is most worth describing — start with underperformers (explain why: role intact + finishing cold, role downgraded, NHL team struggling), then anyone who's been scratched or whose NHL team is out, then the pieces that are producing. Format each line as:
- **Player Name** (TEAM) — one clause describing what happened so far (grade, actual vs expected, TOI trend, series state), one clause describing the likely rest-of-run path given the projection and their NHL team's expected remaining games. Do not recommend anything. Do not say "worth holding" or "give it time" — the reader is holding regardless.

### What to Expect
One paragraph, 2–4 sentences. Describe the probable trajectory for the rest of the playoffs given the concentration, the projections, and the bracket state. If a rostered NHL team is one series away from elimination, name it. If the projection math says the roster's ceiling is capped by which stacks survive round one, say so. No action items.

Hard rules:
- Only use stats, names, and facts from the HEADLINE and FULL PAYLOAD. Never invent injuries, lines, or news — the payload's bucket + active_prob are the only signals you have about availability. If active_prob is below 1.0, the player is "not appearing in the lineup" or "absent from the lineup" — do not guess at "injured" or "IR".
- Speak TO the caller — second person ("you", "your roster"). Never address them by their fantasy team name.
- Be honest when the verdict is bad. A boring truthful read beats a hyped one.
- Keep it specific. Name players. Cite games played, grades, TOI trends, actual vs expected points. No generic "your team has upside" phrasing.
- Never reference the model host, the app, the prompt, or the generation process.
- Do not emit horizontal rules (`---`, `***`, `___`) between sections. The three `### Heading` lines are the only section separators — the UI draws its own visual divider."#;
