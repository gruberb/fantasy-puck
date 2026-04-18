use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post, put},
    Router,
};

use crate::api::handlers;
use crate::config::Config;
use crate::nhl_api::nhl::NhlClient;
use crate::ws::draft_hub::DraftHub;
use crate::FantasyDb;

// Create a shared state type for our API
pub struct AppState {
    pub db: FantasyDb,
    pub nhl_client: NhlClient,
    pub config: Arc<Config>,
    pub draft_hub: DraftHub,
}

// Create the router
pub fn create_router(db: FantasyDb, nhl_client: NhlClient, config: Arc<Config>) -> Router {
    // Create shared application state
    let state = Arc::new(AppState {
        db,
        nhl_client,
        config,
        draft_hub: DraftHub::new(),
    });

    Router::new()
        // ---------------------------------------------------------------
        // Health Checks
        // ---------------------------------------------------------------
        .route("/health/live", get(|| async { StatusCode::OK }))
        .route("/health/ready", get(health_ready))
        // ---------------------------------------------------------------
        // Auth Routes
        // ---------------------------------------------------------------
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/register", post(handlers::auth::register))
        .route("/api/auth/me", get(handlers::auth::get_me))
        .route("/api/auth/profile", put(handlers::auth::update_profile))
        .route("/api/auth/account", delete(handlers::auth::delete_account))
        .route(
            "/api/auth/memberships",
            get(handlers::auth::get_memberships),
        )
        // ---------------------------------------------------------------
        // League Routes
        // ---------------------------------------------------------------
        .route(
            "/api/leagues",
            get(handlers::leagues::list_leagues).post(handlers::leagues::create_league),
        )
        .route(
            "/api/leagues/{league_id}",
            delete(handlers::leagues::delete_league),
        )
        .route(
            "/api/leagues/{league_id}/members",
            get(handlers::leagues::get_league_members),
        )
        .route(
            "/api/leagues/{league_id}/join",
            post(handlers::leagues::join_league),
        )
        .route(
            "/api/leagues/{league_id}/members/{member_id}",
            delete(handlers::leagues::remove_member),
        )
        // ---------------------------------------------------------------
        // Draft Routes
        // ---------------------------------------------------------------
        .route(
            "/api/leagues/{league_id}/draft",
            get(handlers::draft::get_draft_by_league)
                .post(handlers::draft::create_draft_session),
        )
        .route(
            "/api/leagues/{league_id}/draft/randomize-order",
            post(handlers::draft::randomize_order),
        )
        .route(
            "/api/draft/{draft_id}",
            get(handlers::draft::get_draft_state)
                .delete(handlers::draft::delete_draft),
        )
        .route(
            "/api/draft/{draft_id}/populate",
            post(handlers::draft::populate_player_pool),
        )
        .route(
            "/api/draft/{draft_id}/start",
            post(handlers::draft::start_draft),
        )
        .route(
            "/api/draft/{draft_id}/pause",
            post(handlers::draft::pause_draft),
        )
        .route(
            "/api/draft/{draft_id}/resume",
            post(handlers::draft::resume_draft),
        )
        .route(
            "/api/draft/{draft_id}/pick",
            post(handlers::draft::make_pick),
        )
        .route(
            "/api/draft/{draft_id}/finalize",
            post(handlers::draft::finalize_draft),
        )
        .route(
            "/api/draft/{draft_id}/complete",
            post(handlers::draft::complete_draft),
        )
        .route(
            "/api/draft/{draft_id}/sleepers",
            get(handlers::draft::get_eligible_sleepers),
        )
        .route(
            "/api/draft/{draft_id}/sleeper-picks",
            get(handlers::draft::get_sleeper_picks),
        )
        .route(
            "/api/draft/{draft_id}/sleeper/start",
            post(handlers::draft::start_sleeper_round),
        )
        .route(
            "/api/draft/{draft_id}/sleeper/pick",
            post(handlers::draft::make_sleeper_pick),
        )
        // ---------------------------------------------------------------
        // Fantasy Team Routes
        // ---------------------------------------------------------------
        .route("/api/fantasy/teams", get(handlers::teams::list_teams))
        .route(
            "/api/fantasy/teams/{id}",
            get(handlers::teams::get_team)
                .put(handlers::teams::update_team_name),
        )
        .route(
            "/api/fantasy/teams/{id}/players",
            post(handlers::teams::add_player_to_team),
        )
        .route(
            "/api/fantasy/players/{player_id}",
            delete(handlers::teams::remove_player),
        )
        .route(
            "/api/fantasy/rankings",
            get(handlers::rankings::get_rankings),
        )
        .route(
            "/api/fantasy/rankings/daily",
            get(handlers::rankings::get_daily_rankings),
        )
        .route(
            "/api/fantasy/rankings/playoffs",
            get(handlers::rankings::get_playoff_rankings),
        )
        .route(
            "/api/fantasy/team-bets",
            get(handlers::teams::get_team_bets),
        )
        .route(
            "/api/fantasy/players",
            get(handlers::players::get_players_per_team),
        )
        .route(
            "/api/fantasy/team-stats",
            get(handlers::team_stats::get_team_stats),
        )
        // ---------------------------------------------------------------
        // NHL Data
        // ---------------------------------------------------------------
        .route(
            "/api/nhl/skaters/top",
            get(handlers::stats::get_top_skaters),
        )
        .route("/api/nhl/games", get(handlers::games::list_games))
        .route(
            "/api/nhl/playoffs",
            get(handlers::playoffs::get_playoff_info),
        )
        .route("/api/nhl/match-day", get(handlers::games::get_match_day))
        .route(
            "/api/nhl/roster/{team}",
            get(handlers::nhl_rosters::get_team_roster),
        )
        // ---------------------------------------------------------------
        // Sleepers
        // ---------------------------------------------------------------
        .route(
            "/api/fantasy/sleepers",
            get(handlers::sleepers::get_sleepers),
        )
        .route(
            "/api/fantasy/sleepers/{sleeper_id}",
            delete(handlers::sleepers::remove_sleeper),
        )
        // ---------------------------------------------------------------
        // Insights
        // ---------------------------------------------------------------
        .route(
            "/api/insights",
            get(handlers::insights::get_insights),
        )
        // ---------------------------------------------------------------
        // Pulse (me-focused live dashboard)
        // ---------------------------------------------------------------
        .route(
            "/api/pulse",
            get(handlers::pulse::get_pulse),
        )
        // ---------------------------------------------------------------
        // Race Odds (Monte Carlo fantasy-race simulator)
        // ---------------------------------------------------------------
        .route(
            "/api/race-odds",
            get(handlers::race_odds::get_race_odds),
        )
        // ---------------------------------------------------------------
        // Admin
        // ---------------------------------------------------------------
        .route(
            "/api/admin/process-rankings/{date}",
            get(handlers::admin::process_rankings),
        )
        .route(
            "/api/admin/cache/invalidate",
            get(handlers::admin::invalidate_cache),
        )
        .route(
            "/api/admin/backfill-historical",
            get(handlers::admin::backfill_historical_playoffs),
        )
        .route(
            "/api/admin/rebackfill-carousel",
            get(handlers::admin::rebackfill_carousel),
        )
        .route(
            "/api/admin/calibrate",
            get(handlers::admin::calibrate),
        )
        // ---------------------------------------------------------------
        // WebSocket
        // ---------------------------------------------------------------
        .route(
            "/ws/draft/{session_id}",
            get(crate::ws::handler::ws_draft),
        )
        .with_state(state)
}

async fn health_ready(
    State(state): State<Arc<AppState>>,
) -> StatusCode {
    match state.db.ping().await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}
