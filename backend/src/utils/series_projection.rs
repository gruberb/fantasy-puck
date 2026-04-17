use serde::{Deserialize, Serialize};

/// Coarse-grained series leverage bucket used for UI coloring.
/// Red → green axis, seven states. Fits the brutalist palette cleanly.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SeriesStateCode {
    /// This team has already been beaten in the series (opponent has 4 wins).
    Eliminated,
    /// Down 0-3 or 1-3 — one loss from elimination.
    FacingElim,
    /// Trailing but not yet on the brink (0-2, 1-2, 2-3).
    Trailing,
    /// Tied series (0-0, 1-1, 2-2, 3-3).
    Tied,
    /// Leading by one game (1-0, 2-1, 3-2).
    Leading,
    /// One win from advancing (3-0, 3-1).
    AboutToAdvance,
    /// This team has already won the series (this team has 4 wins).
    Advanced,
}

impl SeriesStateCode {
    /// Short headline copy for a cell: "2-1, closing in", "1-3, facing elim", etc.
    pub fn label(&self, wins: u32, opp_wins: u32) -> String {
        match self {
            Self::Advanced => format!("{}-{} advanced", wins, opp_wins),
            Self::Eliminated => format!("{}-{} eliminated", wins, opp_wins),
            Self::AboutToAdvance => format!("{}-{} closing in", wins, opp_wins),
            Self::FacingElim => format!("{}-{} facing elim", wins, opp_wins),
            Self::Leading => format!("{}-{} leading", wins, opp_wins),
            Self::Trailing => format!("{}-{} trailing", wins, opp_wins),
            Self::Tied => format!("{}-{} tied", wins, opp_wins),
        }
    }
}

/// Classify a best-of-7 series state from this team's perspective.
pub fn classify(wins: u32, opp_wins: u32) -> SeriesStateCode {
    if wins >= 4 {
        return SeriesStateCode::Advanced;
    }
    if opp_wins >= 4 {
        return SeriesStateCode::Eliminated;
    }
    if wins == 3 && opp_wins < 3 {
        return SeriesStateCode::AboutToAdvance;
    }
    if opp_wins == 3 && wins < 3 {
        return SeriesStateCode::FacingElim;
    }
    match wins.cmp(&opp_wins) {
        std::cmp::Ordering::Equal => SeriesStateCode::Tied,
        std::cmp::Ordering::Greater => SeriesStateCode::Leading,
        std::cmp::Ordering::Less => SeriesStateCode::Trailing,
    }
}

/// Historical probability (0.0-1.0) that a team wins a best-of-7 from the
/// given state. Based on NHL series outcomes: ~5% when down 0-3, ~50% tied,
/// ~95% when up 3-0. Not a simulation, but honest for a small friends' pool
/// and requires no external data.
pub fn probability_to_advance(wins: u32, opp_wins: u32) -> f32 {
    if wins >= 4 {
        return 1.0;
    }
    if opp_wins >= 4 {
        return 0.0;
    }
    match (wins, opp_wins) {
        (0, 0) | (1, 1) | (2, 2) | (3, 3) => 0.50,
        (1, 0) => 0.62,
        (2, 0) => 0.82,
        (3, 0) => 0.95,
        (2, 1) => 0.65,
        (3, 1) => 0.90,
        (3, 2) => 0.72,
        (0, 1) => 0.38,
        (0, 2) => 0.18,
        (0, 3) => 0.05,
        (1, 2) => 0.35,
        (1, 3) => 0.10,
        (2, 3) => 0.28,
        _ => 0.50,
    }
}

/// Games remaining in a best-of-7 series from the current state.
/// Returns the maximum possible — the series can end earlier if one side clinches.
pub fn games_remaining(wins: u32, opp_wins: u32) -> u32 {
    if wins >= 4 || opp_wins >= 4 {
        return 0;
    }
    7u32.saturating_sub(wins).saturating_sub(opp_wins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify() {
        assert_eq!(classify(4, 0), SeriesStateCode::Advanced);
        assert_eq!(classify(0, 4), SeriesStateCode::Eliminated);
        assert_eq!(classify(3, 0), SeriesStateCode::AboutToAdvance);
        assert_eq!(classify(3, 1), SeriesStateCode::AboutToAdvance);
        assert_eq!(classify(0, 3), SeriesStateCode::FacingElim);
        assert_eq!(classify(1, 3), SeriesStateCode::FacingElim);
        assert_eq!(classify(2, 2), SeriesStateCode::Tied);
        assert_eq!(classify(0, 0), SeriesStateCode::Tied);
        assert_eq!(classify(2, 1), SeriesStateCode::Leading);
        assert_eq!(classify(1, 2), SeriesStateCode::Trailing);
    }

    #[test]
    fn test_probability() {
        assert!(probability_to_advance(0, 3) < 0.10);
        assert_eq!(probability_to_advance(2, 2), 0.50);
        assert!(probability_to_advance(3, 0) > 0.90);
        assert_eq!(probability_to_advance(4, 0), 1.0);
        assert_eq!(probability_to_advance(0, 4), 0.0);
    }

    #[test]
    fn test_games_remaining() {
        assert_eq!(games_remaining(0, 0), 7);
        assert_eq!(games_remaining(2, 2), 3);
        assert_eq!(games_remaining(3, 1), 3);
        assert_eq!(games_remaining(3, 3), 1);
        assert_eq!(games_remaining(4, 0), 0);
    }
}
