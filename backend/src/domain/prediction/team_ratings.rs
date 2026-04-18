//! Team-strength rating derived from the NHL standings feed, blending
//! regular-season points with a recent-form signal.
//!
//! Formula: `rating = 0.7 * season_points + 0.3 * (L10_points_per_game * 82)`.
//!
//! Pre-playoffs the L10 block is a live 10-game rolling window, so a hot
//! team's rating rises a few points above its season mark and a cold
//! team's drops. Once the playoffs begin the standings feed freezes L10 at
//! its final regular-season value, so the blend collapses toward plain RS
//! points — which is correct: playoff-form signal is already carried
//! through the Monte Carlo's per-series starting state, and double-counting
//! it here would over-weight a small sample.
//!
//! Shared by the race-odds handler (fantasy-race sim) and the insights
//! handler (bracket enrichment) so both surfaces report the same strength
//! numbers.

use std::collections::HashMap;

/// Weight applied to season-long standings points in the blended rating.
pub const SEASON_WEIGHT: f32 = 0.7;
/// Weight applied to the 82-game-extrapolated L10 points-per-game value.
pub const RECENT_WEIGHT: f32 = 0.3;

/// Parse the NHL `/standings` JSON and return `abbrev → rating` for every
/// team that has a valid points entry. Teams missing L10 data fall back to
/// their plain season points so we never corrupt the rating with a zero.
pub fn from_standings(root: &serde_json::Value) -> HashMap<String, f32> {
    let Some(arr) = root.get("standings").and_then(|v| v.as_array()) else {
        return HashMap::new();
    };
    arr.iter()
        .filter_map(|entry| {
            let abbrev = entry
                .get("teamAbbrev")
                .and_then(|a| a.get("default"))
                .and_then(|a| a.as_str())?
                .to_string();
            let season_points =
                entry.get("points").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let rating = blend(entry, season_points);
            Some((abbrev, rating))
        })
        .collect()
}

fn blend(entry: &serde_json::Value, season_points: f32) -> f32 {
    let l10_w = entry.get("l10Wins").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let l10_l = entry.get("l10Losses").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let l10_otl = entry
        .get("l10OtLosses")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let l10_games = l10_w + l10_l + l10_otl;
    // No L10 sample → fall back to pure season points. Avoids corrupting
    // early-season ratings (or broken API responses) with a 0 extrapolation.
    if l10_games <= 0.0 {
        return season_points;
    }
    let l10_points = (l10_w * 2.0 + l10_otl) as f32;
    let ppg = l10_points / l10_games as f32;
    let extrapolated = ppg * 82.0;
    SEASON_WEIGHT * season_points + RECENT_WEIGHT * extrapolated
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(abbrev: &str, points: i64, w: i64, l: i64, otl: i64) -> serde_json::Value {
        json!({
            "teamAbbrev": { "default": abbrev },
            "points": points,
            "l10Wins": w,
            "l10Losses": l,
            "l10OtLosses": otl,
        })
    }

    #[test]
    fn average_l10_nudges_rating_toward_extrapolation() {
        // Team with 100 season points and a 5-4-1 L10 (11 pts in 10 games
        // → 90.2 extrapolated over 82). Blended: 0.7·100 + 0.3·90.2 ≈ 97.1.
        let root = json!({ "standings": [entry("ABC", 100, 5, 4, 1)] });
        let map = from_standings(&root);
        let rating = map.get("ABC").copied().unwrap_or_default();
        assert!((rating - 97.06).abs() < 0.5, "got {rating}");
    }

    #[test]
    fn hot_l10_lifts_rating_above_season() {
        // 100 RS pts + 8-1-1 L10 (17 pts → 139.4 extrapolated) → ~111.8.
        let root = json!({ "standings": [entry("HOT", 100, 8, 1, 1)] });
        let rating = from_standings(&root).get("HOT").copied().unwrap_or_default();
        assert!(rating > 108.0, "hot team should rise above season pts, got {rating}");
        assert!(rating < 115.0, "blend should damp, not replace; got {rating}");
    }

    #[test]
    fn cold_l10_drops_rating_below_season() {
        // 100 RS pts + 2-7-1 L10 (5 pts → 41 extrapolated) → ~82.3.
        let root = json!({ "standings": [entry("COLD", 100, 2, 7, 1)] });
        let rating = from_standings(&root).get("COLD").copied().unwrap_or_default();
        assert!(rating < 85.0, "cold team should drop below season pts, got {rating}");
        assert!(rating > 78.0, "blend should damp, not replace; got {rating}");
    }

    #[test]
    fn missing_l10_falls_back_to_season_points() {
        let root = json!({
            "standings": [
                { "teamAbbrev": { "default": "BARE" }, "points": 92 },
            ]
        });
        let rating = from_standings(&root).get("BARE").copied().unwrap_or_default();
        assert!((rating - 92.0).abs() < 1e-5);
    }

    #[test]
    fn empty_standings_returns_empty_map() {
        let root = json!({});
        assert!(from_standings(&root).is_empty());
        let root = json!({ "standings": [] });
        assert!(from_standings(&root).is_empty());
    }
}
