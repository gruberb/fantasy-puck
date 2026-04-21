//! Shared composition of the per-team playoff breakdown.
//!
//! One entry point — [`compose_team_breakdown`] — that both the
//! fantasy-team detail handler and the Pulse handler call with a
//! resolved roster. Returns the full per-player breakdown, team
//! totals, and the descriptive diagnosis (including the Claude
//! narrative) so the caller can embed it in whatever response shape
//! they own.
//!
//! Lives under `handlers/` instead of `domain/` because it reaches
//! into `AppState` (DB pool, cache, narrator port, NHL client for
//! logo URLs). The pure pieces it stitches together already live in
//! `domain::prediction::grade` and `domain::prediction::carousel`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::api::dtos::teams::{
    PlayerBreakdown, PlayerRecentGameCell, PlayerStatsResponse, TeamConcentrationCell,
    TeamDiagnosis, TeamPointsResponse, TeamTotalsResponse,
};
use crate::api::handlers::insights::hockey_today;
use crate::api::routes::AppState;
use crate::api::{current_date_window, game_type, season};
use crate::domain::models::db::FantasyPlayer;
use crate::domain::models::fantasy::PlayerStats;
use crate::domain::prediction::carousel::games_played_from_carousel;
use crate::domain::prediction::grade::{
    classify_bucket, grade, remaining_impact, PlayerBucket,
};
use crate::domain::prediction::player_projection::{PlayerInput, Projection};
use crate::domain::prediction::race_sim::NhlTeamOdds;
use crate::domain::prediction::series_projection::{classify, SeriesStateCode};
use crate::error::Result;
use crate::infra::db::nhl_mirror::{
    self, LeagueTeamSeasonTotalsRow, PlayerPlayoffRollupRow, PlayerRecentGameRow,
};
use crate::infra::prediction::{project_players, race_odds_cache};

pub struct PlayoffBreakdownBundle {
    pub players: Vec<PlayerStatsResponse>,
    pub team_totals: TeamTotalsResponse,
    pub diagnosis: TeamDiagnosis,
}

pub async fn compose_team_breakdown(
    state: &Arc<AppState>,
    league_id: &str,
    team_id: i64,
    team_name: &str,
    players: &[FantasyPlayer],
) -> Result<PlayoffBreakdownBundle> {
    let pool = state.db.pool();
    let season_num = season();
    let today = hockey_today();
    let nhl_ids: Vec<i64> = players.iter().map(|p| p.nhl_id).collect();

    let (rollup_rows, recent_rows, carousel, season_rs, league_totals) = tokio::try_join!(
        nhl_mirror::list_player_playoff_rollup(
            pool,
            &nhl_ids,
            season_num as i32,
            current_date_window(),
        ),
        nhl_mirror::list_player_recent_games(pool, &nhl_ids, season_num as i32, 5),
        async { nhl_mirror::get_playoff_carousel(pool, season_num as i32).await },
        nhl_mirror::list_skater_season_stats(pool, season_num as i32, 2),
        nhl_mirror::list_league_team_season_totals(
            pool,
            league_id,
            season_num as i32,
            3,
            current_date_window(),
        ),
    )?;

    let rollup_by_id: HashMap<i64, PlayerPlayoffRollupRow> =
        rollup_rows.into_iter().map(|r| (r.player_id, r)).collect();
    let mut recent_by_id: HashMap<i64, Vec<PlayerRecentGameRow>> = HashMap::new();
    for r in recent_rows {
        recent_by_id.entry(r.player_id).or_default().push(r);
    }

    let team_games_played = games_played_from_carousel(carousel.as_ref());
    let series_states = build_series_states(carousel.as_ref());

    let rs_points_by_id: HashMap<i64, i32> = season_rs
        .into_iter()
        .map(|r| (r.player_id, r.points))
        .collect();

    let projection_inputs: Vec<PlayerInput> = players
        .iter()
        .filter(|p| !p.position.eq_ignore_ascii_case("G"))
        .map(|p| PlayerInput {
            nhl_id: p.nhl_id,
            player_name: p.name.clone(),
            nhl_team: p.nhl_team.clone(),
            rs_points: rs_points_by_id.get(&p.nhl_id).copied().unwrap_or(0),
        })
        .collect();
    let projections =
        project_players(&state.db, season_num, &projection_inputs, &team_games_played)
            .await
            .unwrap_or_default();

    let nhl_team_odds =
        race_odds_cache::load_nhl_team_odds(state, league_id, season_num, game_type(), &today)
            .await;

    let mut team_totals = PlayerStats::default();
    let mut seen: HashSet<i64> = HashSet::new();
    let mut players_out = Vec::with_capacity(players.len());

    for p in players {
        if !seen.insert(p.nhl_id) {
            continue;
        }
        let rollup = rollup_by_id.get(&p.nhl_id);
        let g = rollup.map(|r| r.goals as i32).unwrap_or(0);
        let a = rollup.map(|r| r.assists as i32).unwrap_or(0);
        let pts = g + a;

        team_totals.goals += g;
        team_totals.assists += a;
        team_totals.total_points += pts;

        let breakdown = build_player_breakdown(
            rollup,
            recent_by_id.get(&p.nhl_id).map(|v| v.as_slice()).unwrap_or(&[]),
            projections.get(&p.nhl_id).copied(),
            nhl_team_odds.get(&p.nhl_team),
            team_games_played.get(&p.nhl_team).copied().unwrap_or(0),
            series_states.get(&p.nhl_team).copied(),
            pts,
        );

        players_out.push(PlayerStatsResponse {
            name: p.name.clone(),
            nhl_team: p.nhl_team.clone(),
            nhl_id: p.nhl_id,
            position: p.position.clone(),
            goals: g,
            assists: a,
            total_points: pts,
            image_url: state.nhl_client.get_player_image_url(p.nhl_id),
            team_logo: state.nhl_client.get_team_logo_url(&p.nhl_team),
            breakdown: Some(breakdown),
        });
    }

    let team_totals_out = TeamTotalsResponse {
        goals: team_totals.goals,
        assists: team_totals.assists,
        total_points: team_totals.total_points,
    };

    let diagnosis_stub = build_diagnosis_stub(team_name, team_id, &league_totals, &players_out);
    let diagnosis_narrative = resolve_team_diagnosis_narrative(
        state,
        league_id,
        team_id,
        &today,
        &TeamPointsResponse {
            team_id,
            team_name: team_name.to_string(),
            players: players_out.clone(),
            team_totals: team_totals_out.clone(),
            diagnosis: Some(diagnosis_stub.clone()),
        },
    )
    .await
    .unwrap_or_default();

    let mut diagnosis = diagnosis_stub;
    diagnosis.narrative_markdown = diagnosis_narrative;

    Ok(PlayoffBreakdownBundle {
        players: players_out,
        team_totals: team_totals_out,
        diagnosis,
    })
}

// --------------------------------------------------------------------
// Internals
// --------------------------------------------------------------------

fn build_series_states(
    carousel: Option<&crate::domain::models::nhl::PlayoffCarousel>,
) -> HashMap<String, SeriesStateCode> {
    let mut out = HashMap::new();
    let Some(c) = carousel else {
        return out;
    };
    for round in &c.rounds {
        for s in &round.series {
            let top_state = classify(s.top_seed.wins as u32, s.bottom_seed.wins as u32);
            let bottom_state = classify(s.bottom_seed.wins as u32, s.top_seed.wins as u32);
            // A team can appear in multiple rounds — the later round's
            // state supersedes the earlier one.
            out.insert(s.top_seed.abbrev.clone(), top_state);
            out.insert(s.bottom_seed.abbrev.clone(), bottom_state);
        }
    }
    out
}

fn build_player_breakdown(
    rollup: Option<&PlayerPlayoffRollupRow>,
    recent: &[PlayerRecentGameRow],
    projection: Option<Projection>,
    nhl_odds: Option<&NhlTeamOdds>,
    team_games_played: u32,
    series_state: Option<SeriesStateCode>,
    actual_points: i32,
) -> PlayerBreakdown {
    let games_played = rollup.map(|r| r.games as u32).unwrap_or(0);
    let toi_seconds_per_game = rollup
        .map(|r| {
            if r.games > 0 {
                (r.total_toi_seconds / r.games) as i32
            } else {
                0
            }
        })
        .unwrap_or(0);

    let projection = projection.unwrap_or(Projection {
        ppg: 0.0,
        active_prob: 1.0,
        toi_multiplier: 1.0,
    });

    let series = series_state.unwrap_or(SeriesStateCode::Tied);
    let grade_report = grade(projection.ppg, games_played, actual_points);
    let eliminated = series == SeriesStateCode::Eliminated;
    let remaining = remaining_impact(
        projection.ppg,
        nhl_odds.map(|o| o.expected_games),
        team_games_played,
        eliminated,
    );
    let bucket: PlayerBucket =
        if games_played == 0 && projection.active_prob >= 1.0 && !eliminated {
            PlayerBucket::TooEarly
        } else {
            classify_bucket(&grade_report, &projection, series)
        };

    let recent_games = recent
        .iter()
        .map(|r| PlayerRecentGameCell {
            game_date: r.game_date.format("%Y-%m-%d").to_string(),
            opponent: r.opponent.clone(),
            toi_seconds: r.toi_seconds,
            goals: r.goals,
            assists: r.assists,
            points: r.points,
        })
        .collect();

    PlayerBreakdown {
        games_played,
        sog: rollup.map(|r| r.sog as i32).unwrap_or(0),
        pim: rollup.map(|r| r.pim as i32).unwrap_or(0),
        plus_minus: rollup.map(|r| r.plus_minus as i32).unwrap_or(0),
        hits: rollup.map(|r| r.hits as i32).unwrap_or(0),
        toi_seconds_per_game,
        projected_ppg: projection.ppg,
        active_prob: projection.active_prob,
        toi_multiplier: projection.toi_multiplier,
        grade: grade_report,
        remaining_impact: remaining,
        series_state: series,
        bucket,
        recent_games,
    }
}

fn build_diagnosis_stub(
    team_name: &str,
    team_id: i64,
    league_totals: &[LeagueTeamSeasonTotalsRow],
    players: &[PlayerStatsResponse],
) -> TeamDiagnosis {
    let league_size = league_totals.len() as i32;
    let (league_rank, my_total) = league_totals
        .iter()
        .enumerate()
        .find(|(_, r)| r.team_id == team_id)
        .map(|(i, r)| (i as i32 + 1, r.points as i32))
        .unwrap_or((league_size, 0));

    let first = league_totals.first().map(|r| r.points as i32).unwrap_or(0);
    let third = league_totals
        .get(2)
        .map(|r| r.points as i32)
        .unwrap_or(first);
    let gap_to_first = (first - my_total).max(0);
    let gap_to_third = my_total - third;

    let mut by_team: HashMap<String, (i32, i32)> = HashMap::new();
    for p in players {
        let entry = by_team.entry(p.nhl_team.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += p.total_points;
    }
    let mut concentration: Vec<TeamConcentrationCell> = by_team
        .into_iter()
        .map(|(nhl_team, (rostered, points))| TeamConcentrationCell {
            nhl_team,
            rostered,
            team_playoff_points: points,
        })
        .collect();
    concentration.sort_by(|a, b| {
        b.rostered
            .cmp(&a.rostered)
            .then_with(|| b.team_playoff_points.cmp(&a.team_playoff_points))
            .then_with(|| a.nhl_team.cmp(&b.nhl_team))
    });

    let headline = format!(
        "{} · #{} of {} · down {} to 1st",
        team_name.to_uppercase(),
        league_rank,
        league_size,
        gap_to_first
    );

    TeamDiagnosis {
        headline,
        narrative_markdown: String::new(),
        league_rank,
        league_size,
        gap_to_first,
        gap_to_third,
        concentration_by_team: concentration,
    }
}

async fn resolve_team_diagnosis_narrative(
    state: &Arc<AppState>,
    league_id: &str,
    team_id: i64,
    today: &str,
    response: &TeamPointsResponse,
) -> Option<String> {
    let key = format!(
        "team_diagnosis:{}:{}:{}:{}:{}",
        league_id,
        team_id,
        season(),
        game_type(),
        today
    );
    if let Ok(Some(cached)) = state.db.cache().get_cached_response::<String>(&key).await {
        return Some(cached);
    }
    let generated = state.prediction.team_diagnosis(response).await?;
    let _ = state
        .db
        .cache()
        .store_response(&key, today, &generated)
        .await;
    Some(generated)
}
