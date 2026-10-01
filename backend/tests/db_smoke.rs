//! SQL smoke tests against a real, migrated Postgres. Skipped unless
//! `TEST_DATABASE_URL` is set, because `cargo test` alone has no database:
//!
//!   docker run -d --rm --name fp-test-pg -e POSTGRES_PASSWORD=postgres -p 55432:5432 postgres:16
//!   for f in supabase/migrations/*.sql; do docker exec -i fp-test-pg psql -U postgres -q < "$f"; done
//!   TEST_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/postgres cargo test --test db_smoke
//!
//! Each test uses its own ids so they can run in parallel against one
//! database and be re-run without a reset.

use fantasy_hockey::domain::models::nhl::{
    ClubStatsSkater, GameBoxscore, GoalieStatsLeaders, StatsLeaders, TodayGame,
};
use fantasy_hockey::infra::db::nhl_mirror;
use fantasy_hockey::{Error, FantasyDb};
use serde_json::json;

async fn db() -> Option<FantasyDb> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    Some(
        FantasyDb::new(&url)
            .await
            .expect("connect to TEST_DATABASE_URL"),
    )
}

/// A fresh id per test run, so reruns don't collide with old rows.
fn unique_id(base: i64) -> i64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as i64;
    base + nanos % 1_000_000
}

fn game(id: i64, state: &str) -> TodayGame {
    serde_json::from_value(json!({
        "id": id, "season": 20252026, "gameType": 3,
        "startTimeUTC": "2026-05-01T23:00:00Z",
        "venue": { "default": "Arena" },
        "awayTeam": { "id": 1, "abbrev": "BOS" },
        "homeTeam": { "id": 2, "abbrev": "TOR" },
        "gameState": state,
        "easternUTCOffset": "-04:00"
    }))
    .unwrap()
}

#[tokio::test]
async fn upsert_game_reports_only_state_changes() {
    let Some(db) = db().await else { return };
    let pool = db.pool();
    let id = unique_id(900_000_000);

    assert!(
        nhl_mirror::upsert_game(pool, &game(id, "FUT"), "2026-05-01")
            .await
            .unwrap()
    );
    assert!(
        !nhl_mirror::upsert_game(pool, &game(id, "FUT"), "2026-05-01")
            .await
            .unwrap()
    );
    assert!(
        nhl_mirror::upsert_game(pool, &game(id, "LIVE"), "2026-05-01")
            .await
            .unwrap()
    );
    assert_eq!(
        nhl_mirror::get_game_state(pool, id)
            .await
            .unwrap()
            .as_deref(),
        Some("LIVE")
    );
}

#[tokio::test]
async fn boxscore_batch_writes_players_and_score() {
    let Some(db) = db().await else { return };
    let pool = db.pool();
    let id = unique_id(910_000_000);
    nhl_mirror::upsert_game(pool, &game(id, "LIVE"), "2026-05-01")
        .await
        .unwrap();

    let player = |pid: i64, g: i32, toi: Option<&str>| {
        json!({ "playerId": pid, "sweaterNumber": 1, "name": { "default": "P" }, "position": "C",
                "goals": g, "assists": 1, "toi": toi })
    };
    let boxscore: GameBoxscore = serde_json::from_value(json!({
        "playerByGameStats": {
            "homeTeam": { "forwards": [player(1, 2, Some("18:24"))], "defense": [player(2, 1, None)], "goalies": [] },
            "awayTeam": { "forwards": [player(3, 0, Some("12:00"))], "defense": [], "goalies": [player(4, 0, None)] }
        }
    }))
    .unwrap();

    let n = nhl_mirror::upsert_boxscore_players(pool, id, "TOR", "BOS", &boxscore)
        .await
        .unwrap();
    assert_eq!(n, 4);
    // Re-running the upsert must be idempotent.
    nhl_mirror::upsert_boxscore_players(pool, id, "TOR", "BOS", &boxscore)
        .await
        .unwrap();

    let (home, away, rows): (Option<i32>, Option<i32>, i64) = sqlx::query_as(
        "SELECT g.home_score, g.away_score, (SELECT COUNT(*) FROM nhl_player_game_stats WHERE game_id = $1)
           FROM nhl_games g WHERE g.game_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!((home, away, rows), (Some(3), Some(0), 4));
}

#[tokio::test]
async fn leaderboard_does_not_zero_club_stats_totals() {
    let Some(db) = db().await else { return };
    let pool = db.pool();
    let pid = unique_id(8_900_000);
    let season = 19_000_000 + (pid % 1000) as i32;

    let skater: ClubStatsSkater = serde_json::from_value(json!({
        "playerId": pid, "firstName": { "default": "A" }, "lastName": { "default": "B" },
        "positionCode": "C", "goals": 30, "assists": 40, "points": 70, "shots": 200
    }))
    .unwrap();
    nhl_mirror::upsert_team_club_stats(pool, season, 2, "TOR", &[skater])
        .await
        .unwrap();

    // The player only appears in the TOI category of the leaderboard.
    let leaders: StatsLeaders = serde_json::from_value(json!({
        "goalsSh": [], "plusMinus": [], "assists": [], "goalsPp": [], "faceoffLeaders": [],
        "penaltyMins": [], "goals": [], "points": [],
        "toi": [{ "id": pid, "firstName": { "default": "A" }, "lastName": { "default": "B" },
                  "teamAbbrev": "TOR", "position": "C", "value": 1200.0 }]
    }))
    .unwrap();
    nhl_mirror::upsert_skater_leaderboard(pool, season, 2, &leaders)
        .await
        .unwrap();

    let rows = nhl_mirror::list_skater_season_stats(pool, season, 2)
        .await
        .unwrap();
    let row = rows.iter().find(|r| r.player_id == pid).unwrap();
    assert_eq!((row.goals, row.assists, row.points), (30, 40, 70));
}

#[tokio::test]
async fn goalie_wins_and_raw_standings_round_trip() {
    let Some(db) = db().await else { return };
    let pool = db.pool();
    let pid = unique_id(8_800_000);
    let season = 18_000_000 + (pid % 1000) as i32;

    let goalie = |value: f64| {
        json!([{ "id": pid, "firstName": { "default": "G" }, "lastName": { "default": "K" },
                 "teamAbbrev": "TOR", "position": "G", "value": value }])
    };
    let leaders: GoalieStatsLeaders = serde_json::from_value(json!({
        "wins": goalie(40.0), "savePctg": goalie(0.915)
    }))
    .unwrap();
    nhl_mirror::upsert_goalie_leaderboard(pool, season, 2, &leaders)
        .await
        .unwrap();
    let rows = nhl_mirror::list_goalie_season_stats(pool, season, 2)
        .await
        .unwrap();
    assert_eq!(rows[0].wins, Some(40));

    let standings = json!({ "standings": [{
        "teamAbbrev": { "default": "TOR" }, "points": 100, "gamesPlayed": 82, "wins": 46,
        "losses": 28, "otLosses": 8, "homeWins": 26
    }]});
    assert_eq!(
        nhl_mirror::upsert_standings(pool, season, &standings)
            .await
            .unwrap(),
        1
    );
    let payload = nhl_mirror::load_standings_payload(pool, season)
        .await
        .unwrap();
    assert_eq!(payload["standings"][0]["homeWins"], 26);
}

struct DraftFixture {
    session_id: String,
    league_id: String,
    team_ids: Vec<i64>,
    pool_ids: Vec<String>,
}

async fn draft_fixture(db: &FantasyDb, members: usize, players: usize) -> DraftFixture {
    let pool = db.pool();
    let tag = unique_id(0);
    let league_id: String =
        sqlx::query_scalar("INSERT INTO leagues (name) VALUES ('t') RETURNING id::text")
            .fetch_one(pool)
            .await
            .unwrap();
    let mut team_ids = Vec::new();
    for i in 0..members {
        let user_id: String = sqlx::query_scalar(
            "INSERT INTO users (email, password_hash) VALUES ($1, 'x') RETURNING id::text",
        )
        .bind(format!("u{tag}-{i}-{}@test", league_id))
        .fetch_one(pool)
        .await
        .unwrap();
        let team_id: i64 = sqlx::query_scalar(
            "INSERT INTO fantasy_teams (name, user_id, league_id) VALUES ('t', $1::uuid, $2::uuid) RETURNING id",
        )
        .bind(&user_id)
        .bind(&league_id)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO league_members (league_id, user_id, fantasy_team_id, draft_order) VALUES ($1::uuid, $2::uuid, $3, $4)",
        )
        .bind(&league_id)
        .bind(&user_id)
        .bind(team_id)
        .bind(i as i32)
        .execute(pool)
        .await
        .unwrap();
        team_ids.push(team_id);
    }
    let session_id: String = sqlx::query_scalar(
        "INSERT INTO draft_sessions (league_id, status, total_rounds, sleeper_status)
         VALUES ($1::uuid, 'active', 1, 'active') RETURNING id::text",
    )
    .bind(&league_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut pool_ids = Vec::new();
    for p in 0..players {
        let id: String = sqlx::query_scalar(
            "INSERT INTO player_pool (draft_session_id, nhl_id, name) VALUES ($1::uuid, $2, 'p') RETURNING id::text",
        )
        .bind(&session_id)
        .bind(8_000_000 + p as i64)
        .fetch_one(pool)
        .await
        .unwrap();
        pool_ids.push(id);
    }
    DraftFixture {
        session_id,
        league_id,
        team_ids,
        pool_ids,
    }
}

#[tokio::test]
async fn concurrent_picks_of_one_player_yield_one_pick() {
    let Some(db) = db().await else { return };
    let f = draft_fixture(&db, 2, 2).await;

    let (a, b) = tokio::join!(
        db.make_draft_pick(&f.session_id, &f.pool_ids[0]),
        db.make_draft_pick(&f.session_id, &f.pool_ids[0]),
    );
    let ok = [a.is_ok(), b.is_ok()];
    assert_eq!(
        ok.iter().filter(|x| **x).count(),
        1,
        "exactly one pick should land"
    );
    let err = a.err().or(b.err()).expect("one pick fails");
    assert!(
        matches!(err, Error::Validation(_)),
        "loser gets a validation error: {err:?}"
    );

    // Second slot, then the draft is done (1 round x 2 members).
    let (pick, session) = db
        .make_draft_pick(&f.session_id, &f.pool_ids[1])
        .await
        .unwrap();
    assert_eq!(pick.pick_number, 1);
    assert_eq!(session.status, "picks_done");

    db.finalize_draft_to_players(&f.session_id).await.unwrap();
    let players: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM fantasy_players WHERE team_id = ANY($1)")
            .bind(&f.team_ids)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(players, 2);
}

#[tokio::test]
async fn sleeper_pick_is_one_per_team_and_league_scoped() {
    let Some(db) = db().await else { return };
    let f = draft_fixture(&db, 2, 2).await;

    db.make_sleeper_pick(&f.session_id, f.team_ids[0], &f.pool_ids[0])
        .await
        .unwrap();
    let again = db
        .make_sleeper_pick(&f.session_id, f.team_ids[0], &f.pool_ids[1])
        .await;
    assert!(matches!(again, Err(Error::Validation(_))));
    let taken = db
        .make_sleeper_pick(&f.session_id, f.team_ids[1], &f.pool_ids[0])
        .await;
    assert!(matches!(taken, Err(Error::Validation(_))));

    // The same NHL player is still available as a sleeper in another league.
    let other = draft_fixture(&db, 1, 1).await;
    db.make_sleeper_pick(&other.session_id, other.team_ids[0], &other.pool_ids[0])
        .await
        .unwrap();

    let session = db
        .make_sleeper_pick(&f.session_id, f.team_ids[1], &f.pool_ids[1])
        .await
        .unwrap();
    assert_eq!(session.sleeper_status.as_deref(), Some("completed"));
}

#[tokio::test]
async fn randomize_order_and_rankings_snapshot_run() {
    let Some(db) = db().await else { return };
    let f = draft_fixture(&db, 3, 0).await;
    db.randomize_draft_order(&f.league_id).await.unwrap();
    let mut orders: Vec<i32> =
        sqlx::query_scalar("SELECT draft_order FROM league_members WHERE league_id = $1::uuid")
            .bind(&f.league_id)
            .fetch_all(db.pool())
            .await
            .unwrap();
    orders.sort();
    assert_eq!(orders, vec![0, 1, 2]);

    // No scoring rows exist for this league, so nothing is written, but
    // the statement (TEXT/DATE casts, window function) must run.
    assert_eq!(
        db.snapshot_daily_rankings(&f.league_id, "2026-05-01")
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn new_readers_run_and_respect_finalization() {
    let Some(db) = db().await else { return };
    let pool = db.pool();
    let id = unique_id(920_000_000);
    nhl_mirror::upsert_game(pool, &game(id, "OFF"), "2026-05-01")
        .await
        .unwrap();
    let pid = 7_000_000 + id % 1000;
    sqlx::query(
        "INSERT INTO nhl_player_game_stats (game_id, player_id, team_abbrev, position, name, goals, assists, points)
         VALUES ($1, $2, 'TOR', 'C', 'p', 1, 1, 2)",
    )
    .bind(id)
    .bind(pid)
    .execute(pool)
    .await
    .unwrap();

    let window = fantasy_hockey::infra::db::DateWindow::between("2026-04-18", "2026-06-14");
    let before = nhl_mirror::sum_player_points(pool, &[pid], 20252026, 3, window)
        .await
        .unwrap();
    assert!(before.is_empty(), "unfinalized games don't count");
    nhl_mirror::mark_game_stats_finalized(pool, id)
        .await
        .unwrap();
    let after = nhl_mirror::sum_player_points(pool, &[pid], 20252026, 3, window)
        .await
        .unwrap();
    assert_eq!(after.get(&pid), Some(&2));
    let outside = fantasy_hockey::infra::db::DateWindow::since("2026-05-02");
    assert!(
        nhl_mirror::sum_player_points(pool, &[pid], 20252026, 3, outside)
            .await
            .unwrap()
            .is_empty()
    );

    let started = nhl_mirror::list_started_games(pool, 20252026)
        .await
        .unwrap();
    assert!(started.iter().any(|g| g.game_id == id));
    nhl_mirror::list_skater_leaders(pool, 20252026, 2, 10)
        .await
        .unwrap();
    assert!(nhl_mirror::get_team_roster(pool, "ZZZ", 20252026)
        .await
        .unwrap()
        .is_none());
}
