use std::collections::HashMap;

use sqlx::postgres::PgPool;
use sqlx::Row;

use crate::error::Result;
use crate::domain::models::db::{FantasyTeam, FantasyTeamBets, NhlBetCount, TeamNhlCount};
use crate::domain::models::fantasy::{FantasyTeamInGame, PlayerInGame};

pub struct TeamDbService<'a> {
    pool: &'a PgPool,
}

impl<'a> TeamDbService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Get a fantasy team by ID, verifying it belongs to the given league
    pub async fn get_team(&self, team_id: i64, league_id: &str) -> Result<FantasyTeam> {
        let team = sqlx::query_as::<_, FantasyTeam>(
            r#"
            SELECT DISTINCT ft.id, ft.name
            FROM fantasy_teams ft
            INNER JOIN league_members lm ON lm.fantasy_team_id = ft.id
            WHERE ft.id = $1 AND lm.league_id = $2::uuid
            "#,
        )
        .bind(team_id)
        .bind(league_id)
        .fetch_one(self.pool)
        .await?;

        Ok(team)
    }

    /// Get all fantasy teams in a league
    pub async fn get_all_teams(&self, league_id: &str) -> Result<Vec<FantasyTeam>> {
        let teams = sqlx::query_as::<_, FantasyTeam>(
            r#"
            SELECT DISTINCT ft.id, ft.name
            FROM fantasy_teams ft
            INNER JOIN league_members lm ON lm.fantasy_team_id = ft.id
            WHERE lm.league_id = $1::uuid
            "#,
        )
        .bind(league_id)
        .fetch_all(self.pool)
        .await?;

        Ok(teams)
    }

    /// For each fantasy team in a league, return how many players they have from each NHL team.
    pub async fn get_fantasy_bets_by_nhl_team(
        &self,
        league_id: &str,
    ) -> Result<Vec<FantasyTeamBets>> {
        let rows: Vec<TeamNhlCount> = sqlx::query_as::<_, TeamNhlCount>(
            r#"
            SELECT
                t.id          AS team_id,
                t.name        AS team_name,
                p.nhl_team    AS nhl_team,
                COUNT(*)      AS num_players
            FROM fantasy_teams t
            INNER JOIN league_members lm ON lm.fantasy_team_id = t.id
            JOIN fantasy_players p ON p.team_id = t.id
            WHERE lm.league_id = $1::uuid
            GROUP BY t.id, t.name, p.nhl_team
            ORDER BY t.id
            "#,
        )
        .bind(league_id)
        .fetch_all(self.pool)
        .await?;

        // Group them in Rust by (team_id + team_name)
        let mut map: HashMap<(i64, String), Vec<NhlBetCount>> = HashMap::new();

        for row in rows {
            map.entry((row.team_id, row.team_name.clone()))
                .or_default()
                .push(NhlBetCount {
                    nhl_team: row.nhl_team,
                    num_players: row.num_players,
                });
        }

        // Build a vec of FantasyTeamBets for final output
        let mut result = Vec::with_capacity(map.len());
        for ((team_id, team_name), bets) in map {
            result.push(FantasyTeamBets {
                team_id,
                team_name,
                bets,
            });
        }

        // Sort the final result by team_id
        result.sort_by_key(|entry| entry.team_id);

        Ok(result)
    }

    /// Get fantasy teams (in a league) with players in specified NHL teams
    pub async fn get_fantasy_teams_for_nhl_teams(
        &self,
        nhl_teams: &[&str],
        league_id: &str,
    ) -> Result<Vec<FantasyTeamInGame>> {
        if nhl_teams.is_empty() {
            return Ok(Vec::new());
        }

        // Build parameterized placeholders for the IN clause
        // $1 is league_id, then $2..$N+1 are the nhl teams
        let placeholders = nhl_teams
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 2))
            .collect::<Vec<_>>()
            .join(",");
        let query_str = format!(
            r#"SELECT
                  p.id as player_id,
                  p.nhl_id as nhl_id,
                  p.name as player_name,
                  p.team_id as fantasy_team_id,
                  t.name as fantasy_team_name,
                  p.nhl_team,
                  p.position
               FROM fantasy_players p
               JOIN fantasy_teams t ON p.team_id = t.id
               INNER JOIN league_members lm ON lm.fantasy_team_id = t.id
               WHERE lm.league_id = $1::uuid
               AND p.nhl_team IN ({})"#,
            placeholders
        );

        // Bind league_id first, then each team abbreviation
        let mut query = sqlx::query(&query_str);
        query = query.bind(league_id);
        for team in nhl_teams {
            query = query.bind(team);
        }

        let rows = query
            .map(|row: sqlx::postgres::PgRow| {
                let player_id: i64 = row.get("player_id");
                let nhl_id: i64 = row.get("nhl_id");
                let player_name: String = row.get("player_name");
                let fantasy_team_id: i64 = row.get("fantasy_team_id");
                let fantasy_team_name: String = row.get("fantasy_team_name");
                let nhl_team: String = row.get("nhl_team");
                let position: String = row.get("position");

                (
                    fantasy_team_id,
                    fantasy_team_name,
                    player_id,
                    nhl_id,
                    player_name,
                    nhl_team,
                    position,
                )
            })
            .fetch_all(self.pool)
            .await?;

        // Group by fantasy team
        let mut teams_map: HashMap<i64, FantasyTeamInGame> = HashMap::new();

        for (
            fantasy_team_id,
            fantasy_team_name,
            player_id,
            nhl_id,
            player_name,
            nhl_team,
            position,
        ) in rows
        {
            let team_entry =
                teams_map
                    .entry(fantasy_team_id)
                    .or_insert_with(|| FantasyTeamInGame {
                        team_id: fantasy_team_id,
                        team_name: fantasy_team_name.clone(),
                        players: Vec::new(),
                    });

            team_entry.players.push(PlayerInGame {
                player_id,
                nhl_id,
                player_name,
                nhl_team,
                position,
            });
        }

        let result: Vec<FantasyTeamInGame> = teams_map.into_values().collect();
        Ok(result)
    }

    /// Return last-N-days of team points for every team in the league,
    /// clipped so no row older than `min_date` is returned. Result:
    /// team_id -> Vec<i32> in chronological order (oldest first).
    /// Missing days are absent from each vec rather than padded with zeros.
    ///
    /// `min_date` (YYYY-MM-DD, inclusive) keeps pre-playoff daily_rankings
    /// from leaking into Pulse's "Yesterday" column on day 1 of a new
    /// round — passing `playoff_start()` clears everything before puck
    /// drop. Pass an empty string to disable the clip.
    pub async fn get_team_sparklines(
        &self,
        league_id: &str,
        days: i32,
        min_date: &str,
    ) -> Result<HashMap<i64, Vec<i32>>> {
        let since = chrono::Utc::now()
            - chrono::Duration::days(days as i64);
        let window_start = since.format("%Y-%m-%d").to_string();
        // Take the later of the trailing-N-days window and the caller's
        // `min_date` floor — whichever clips more.
        let since_str: String = if !min_date.is_empty() && min_date > window_start.as_str() {
            min_date.to_string()
        } else {
            window_start
        };

        let rows = sqlx::query(
            r#"
            SELECT team_id, date, points
            FROM daily_rankings
            WHERE league_id = $1::uuid
              AND date >= $2
            ORDER BY team_id, date
            "#,
        )
        .bind(league_id)
        .bind(&since_str)
        .map(|row: sqlx::postgres::PgRow| {
            (
                row.get::<i64, _>("team_id"),
                row.get::<i32, _>("points"),
            )
        })
        .fetch_all(self.pool)
        .await?;

        let mut map: HashMap<i64, Vec<i32>> = HashMap::new();
        for (team_id, points) in rows {
            map.entry(team_id).or_default().push(points);
        }
        Ok(map)
    }

    /// For each fantasy team in the league, return the list of rostered players
    /// in a simple flat structure. Used by the Series Forecast on Pulse.
    pub async fn get_all_teams_with_players(
        &self,
        league_id: &str,
    ) -> Result<Vec<FantasyTeamInGame>> {
        let rows = sqlx::query(
            r#"
            SELECT
                t.id          AS team_id,
                t.name        AS team_name,
                p.id          AS player_id,
                p.nhl_id      AS nhl_id,
                p.name        AS player_name,
                p.nhl_team    AS nhl_team,
                p.position    AS position
            FROM fantasy_teams t
            INNER JOIN league_members lm ON lm.fantasy_team_id = t.id
            LEFT JOIN fantasy_players p ON p.team_id = t.id
            WHERE lm.league_id = $1::uuid
            ORDER BY t.id, p.id
            "#,
        )
        .bind(league_id)
        .map(|row: sqlx::postgres::PgRow| {
            let team_id: i64 = row.get("team_id");
            let team_name: String = row.get("team_name");
            let player_id: Option<i64> = row.try_get("player_id").ok();
            let nhl_id: Option<i64> = row.try_get("nhl_id").ok();
            let player_name: Option<String> = row.try_get("player_name").ok();
            let nhl_team: Option<String> = row.try_get("nhl_team").ok();
            let position: Option<String> = row.try_get("position").ok();
            (team_id, team_name, player_id, nhl_id, player_name, nhl_team, position)
        })
        .fetch_all(self.pool)
        .await?;

        let mut map: HashMap<i64, FantasyTeamInGame> = HashMap::new();
        for (team_id, team_name, player_id, nhl_id, player_name, nhl_team, position) in rows {
            let entry = map.entry(team_id).or_insert_with(|| FantasyTeamInGame {
                team_id,
                team_name: team_name.clone(),
                players: Vec::new(),
            });
            if let (Some(player_id), Some(nhl_id), Some(player_name), Some(nhl_team), Some(position)) =
                (player_id, nhl_id, player_name, nhl_team, position)
            {
                entry.players.push(PlayerInGame {
                    player_id,
                    nhl_id,
                    player_name,
                    nhl_team,
                    position,
                });
            }
        }

        let mut result: Vec<FantasyTeamInGame> = map.into_values().collect();
        result.sort_by_key(|t| t.team_id);
        Ok(result)
    }

    /// Update a fantasy team's name.
    pub async fn update_team_name(&self, team_id: i64, name: &str) -> Result<()> {
        sqlx::query("UPDATE fantasy_teams SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(team_id)
            .execute(self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_daily_ranking_stats(
        &self,
        league_id: &str,
    ) -> Result<Vec<crate::domain::models::db::TeamDailyRankingStats>> {
        // Check if the table has any rows for this league
        let count: Option<i64> =
            sqlx::query_scalar("SELECT COUNT(*) FROM daily_rankings WHERE league_id = $1::uuid")
                .bind(league_id)
                .fetch_optional(self.pool)
                .await?;

        if count.unwrap_or(0) == 0 {
            return Ok(Vec::new());
        }

        // Build a map of team stats
        let mut team_stats: HashMap<i64, crate::domain::models::db::TeamDailyRankingStats> = HashMap::new();

        // First, initialize team stats with team IDs for this league
        let team_ids: Vec<i64> =
            sqlx::query_scalar("SELECT DISTINCT team_id FROM daily_rankings WHERE league_id = $1::uuid")
                .bind(league_id)
                .fetch_all(self.pool)
                .await?;

        for team_id in team_ids {
            team_stats.insert(
                team_id,
                crate::domain::models::db::TeamDailyRankingStats {
                    team_id,
                    wins: 0,
                    top_three: 0,
                    win_dates: Vec::new(),
                    top_three_dates: Vec::new(),
                },
            );
        }

        // Query to get all daily rankings for this league with computed rank
        let daily_rankings = sqlx::query(
            r#"
            SELECT
                r1.date,
                r1.team_id,
                r1.daily_points,
                (
                    SELECT COUNT(*) + 1
                    FROM daily_rankings r2
                    WHERE r2.date = r1.date
                    AND r2.league_id = r1.league_id
                    AND r2.daily_points > r1.daily_points
                ) AS true_rank
            FROM daily_rankings r1
            WHERE r1.league_id = $1::uuid
            ORDER BY r1.date, true_rank
            "#,
        )
        .bind(league_id)
        .map(|row: sqlx::postgres::PgRow| {
            (
                row.get::<String, _>("date"),
                row.get::<i64, _>("team_id"),
                row.get::<i64, _>("true_rank"),
            )
        })
        .fetch_all(self.pool)
        .await?;

        // Process all daily rankings
        for (date, team_id, true_rank) in daily_rankings {
            if let Some(stats) = team_stats.get_mut(&team_id) {
                if true_rank == 1 {
                    stats.wins += 1;
                    stats.win_dates.push(date.clone());
                }
                // If this was a 2nd or 3rd place finish (exclusive)
                else if true_rank <= 3 {
                    stats.top_three += 1;
                    stats.top_three_dates.push(date.clone());
                }
            }
        }

        // Convert HashMap to Vec
        Ok(team_stats.into_values().collect())
    }
}

impl<'a> TeamDbService<'a> {
    /// Sparkline that merges the `daily_rankings` historical rollup
    /// with today's live running total from `v_daily_fantasy_totals`.
    /// Returns per-team points for the last `days` days, ordered by
    /// date ascending (so callers can take `.last()` for the latest).
    ///
    /// Why not just `get_team_sparklines`: that reader goes to
    /// `daily_rankings` only, which is populated by the 9am / 3pm UTC
    /// scheduler from *yesterday's* boxscores. On day 1 of a new
    /// round — or on any day before the cron has fired — the chart
    /// is empty even though `nhl_player_game_stats` has today's
    /// data. Unioning the view in gives today's row immediately and
    /// deduplicates against daily_rankings for prior days.
    pub async fn get_team_sparklines_with_live(
        &self,
        league_id: &str,
        days: i32,
        min_date: &str,
    ) -> Result<HashMap<i64, Vec<i32>>> {
        // Visible window is always the last `days` calendar days
        // ending today — the frontend's 5-DAY column needs a stable
        // number of bars regardless of when playoffs started, because
        // with fewer data points `Sparkbars` renders each bar as
        // `width / count` and a single-point series fills the whole
        // box. `min_date` still clamps the SQL read (we don't want to
        // scan pre-playoff daily_rankings) but the returned vector is
        // always zero-padded to `days` entries at the older edge.
        let today = chrono::Utc::now().date_naive();
        let window_start = today - chrono::Duration::days((days - 1).max(0) as i64);
        let min_date_parsed = chrono::NaiveDate::parse_from_str(min_date, "%Y-%m-%d").ok();
        let sql_since = match min_date_parsed {
            Some(d) if d > window_start => d,
            _ => window_start,
        };
        let since_str = sql_since.format("%Y-%m-%d").to_string();

        // Expected-date sequence covers the full visible window, even
        // where the SQL side clamps it shorter — padding fills the
        // older slots with zeros so teams with one scoring day render
        // as five distinct bars (`[0, 0, 0, P, 0]`) instead of one.
        let mut expected_dates: Vec<String> = Vec::new();
        let mut cur = window_start;
        while cur <= today {
            expected_dates.push(cur.format("%Y-%m-%d").to_string());
            cur += chrono::Duration::days(1);
        }

        // Prefer daily_rankings where present (official finalized
        // rollup); fall back to the live view for dates it hasn't
        // covered yet (typically today). DISTINCT ON picks the
        // daily_rankings row over the view row when both exist for
        // the same (team, date).
        let rows = sqlx::query(
            r#"
            SELECT team_id, date, points
              FROM (
                  SELECT DISTINCT ON (team_id, date)
                         team_id, date::text AS date, points, 1 AS src
                    FROM daily_rankings
                   WHERE league_id = $1::uuid
                     AND date >= $2
                UNION ALL
                  SELECT team_id, date::text AS date, points::int AS points, 2 AS src
                    FROM v_daily_fantasy_totals
                   WHERE league_id = $1::uuid
                     AND date::text >= $2
              ) merged
             ORDER BY team_id, date, src
            "#,
        )
        .bind(league_id)
        .bind(&since_str)
        .map(|row: sqlx::postgres::PgRow| {
            (
                row.get::<i64, _>("team_id"),
                row.get::<String, _>("date"),
                row.get::<i32, _>("points"),
            )
        })
        .fetch_all(self.pool)
        .await?;

        // Dedup per (team, date): the outer ORDER BY ensures src=1
        // (daily_rankings) comes first when both sources have the
        // same (team, date). We keep the first row per unique key.
        let mut seen: std::collections::HashSet<(i64, String)> =
            std::collections::HashSet::new();
        let mut by_team_date: HashMap<i64, HashMap<String, i32>> = HashMap::new();
        for (team_id, date, points) in rows {
            if seen.insert((team_id, date.clone())) {
                by_team_date.entry(team_id).or_default().insert(date, points);
            }
        }

        // Zero-pad each team's vector against the full expected date
        // sequence. A team scoring only on day 3 of a 5-day window
        // returns `[0, 0, P, 0, 0]`, which the Sparkbars component
        // renders as five distinct bars instead of one solid block.
        let map: HashMap<i64, Vec<i32>> = by_team_date
            .into_iter()
            .map(|(team_id, dates)| {
                let padded: Vec<i32> = expected_dates
                    .iter()
                    .map(|d| dates.get(d).copied().unwrap_or(0))
                    .collect();
                (team_id, padded)
            })
            .collect();
        Ok(map)
    }
}
