//! Service tuning constants.
//!
//! Every timeout, retry count, cron schedule, cache TTL, and poller
//! cadence the backend uses at runtime is declared in this file. There
//! is no other place to look.
//!
//! # What belongs here and what does not
//!
//! This file is for values that are part of the service's operating
//! contract — things a maintainer might reasonably want to tune under
//! production load, but that should apply identically to every
//! deployment. Anything that varies between environments (database URL,
//! API keys, NHL season, feature flags) stays in [`crate::config::Config`]
//! and is read from the environment.
//!
//! The line between the two: if a second team running the same code
//! would reasonably pick a different value, it is environment config;
//! if changing the value is a code review, it is tuning.
//!
//! # Conventions
//!
//! - Durations use [`std::time::Duration`] so call sites never see
//!   naked integers. Use second-precision for anything over 10 s and
//!   millisecond-precision for retry backoffs and ping intervals.
//! - Cron schedules use six-field syntax (`sec min hour dom mon dow`)
//!   as accepted by [`tokio_cron_scheduler::Job::new_async`]. Do not
//!   shorten to five fields — the scheduler parses the first field as
//!   seconds and drops jobs silently if the count is wrong.
//! - Every constant has a doc comment. The comment states the unit,
//!   the observable effect of changing the value, and any constraint
//!   relative to other constants (see the retry/timeout pair for an
//!   example).
//!
//! # Cross-cutting constraints
//!
//! - [`http::AXUM_REQUEST_TIMEOUT`] must exceed
//!   [`nhl_client::REQUEST_TIMEOUT`], otherwise the outer Axum
//!   timeout fires before the NHL retry loop completes and the handler
//!   returns 504 on every transient 429.
//! - [`nhl_client::REQUEST_TIMEOUT`] must exceed the sum of the
//!   [`nhl_client::RETRY_INITIAL_DELAY`]-based backoff sequence
//!   (~15 s for five retries at 500 ms base) or the retry budget is
//!   dead code.

use std::time::Duration;

// ---------------------------------------------------------------------
// NHL client (backend/src/nhl_api/nhl.rs)
// ---------------------------------------------------------------------

/// HTTP client that talks to the undocumented NHL API at
/// `api-web.nhle.com`. The API enforces a per-IP rate limit with no
/// public quota; the values in this module are what survives
/// playoff-evening traffic with margin.
pub mod nhl_client {
    use super::Duration;

    /// Maximum in-flight NHL requests from this process at any moment.
    ///
    /// The HTTP layer funnels all outbound calls through a
    /// [`tokio::sync::Semaphore`] of this size. Raising it produces
    /// faster cold loads on fan-out-heavy pages (Games extended,
    /// Insights pre-warm) but pushes more requests into NHL's 429
    /// window. Lowering it serializes the request queue and slows
    /// first render. 10 is the sustainable ceiling we measured during
    /// 2026 playoffs.
    pub const MAX_CONCURRENT_REQUESTS: usize = 10;

    /// Maximum 429 retries per request before giving up with
    /// [`crate::error::Error::NhlApi`].
    ///
    /// Paired with [`RETRY_INITIAL_DELAY`], the worst-case total wait
    /// is the sum of the backoff sequence: ~15 s at five retries and
    /// 500 ms base. Raising this absorbs longer rate-limit windows;
    /// lowering it surfaces errors to the caller sooner so the UI can
    /// degrade faster.
    pub const MAX_RETRIES: u32 = 5;

    /// Base delay for exponential backoff on 429 responses. The delay
    /// doubles between each retry: 500 ms, 1 s, 2 s, 4 s, 8 s.
    pub const RETRY_INITIAL_DELAY: Duration = Duration::from_millis(500);

    /// Per-request timeout. Covers the whole retry-loop plus body
    /// download. Must be at least as long as the worst-case retry
    /// budget (~15 s at current settings) or the retry loop never
    /// gets to complete.
    pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

    /// How often the background task sweeps expired entries out of
    /// the in-process URL cache. Spawned from `main.rs` via
    /// [`crate::infra::nhl::client::NhlClient::start_cache_cleanup`]. Short
    /// enough that expired entries never accumulate; long enough
    /// that the sweep itself is cheap.
    pub const CACHE_CLEANUP_INTERVAL: Duration = Duration::from_secs(300);

    // ---- Per-endpoint TTLs for the in-memory URL cache --------------
    //
    // One TTL per endpoint family. The value is how often the
    // underlying NHL data can change in a way the app cares about,
    // not how often it can change in theory.

    /// Skater and goalie season leaderboards. Updates after every
    /// completed game but rarely more than a handful of times per
    /// day.
    pub const SKATER_STATS_TTL: Duration = Duration::from_secs(300);

    /// `schedule/now` and `schedule/{date}`. Game state transitions
    /// (FUT → LIVE → OFF) are user-visible but not minute-to-minute.
    pub const SCHEDULE_TTL: Duration = Duration::from_secs(120);

    /// `gamecenter/{game_id}/landing` and related per-game derived
    /// endpoints. Lightweight live state; the Games-page opt-in poll
    /// is 30 s, so this governs real NHL call rate.
    pub const GAME_CENTER_TTL: Duration = Duration::from_secs(120);

    /// Boxscore for a game still in progress. The 60 s TTL means
    /// multiple concurrent handlers reading the same live boxscore
    /// produce at most one NHL call per minute.
    pub const BOXSCORE_LIVE_TTL: Duration = Duration::from_secs(60);

    /// Boxscore for a finished game. Effectively immutable; 24 h lets
    /// the same boxscore power the insights narrative, the /games
    /// page, and the rankings cron without re-fetching.
    pub const BOXSCORE_FINAL_TTL: Duration = Duration::from_secs(86_400);

    /// Playoff carousel (bracket shape + series state). Only changes
    /// on a series resolution or a new round. 15 min is the upper
    /// bound on how long a Round N+1 matchup could go unseen after
    /// a clinching goal.
    pub const PLAYOFF_CAROUSEL_TTL: Duration = Duration::from_secs(900);

    /// Per-player game log. The heavy fan-out on Pulse and Insights
    /// (one call per player on the slate) relies on this cache to
    /// amortize cold loads across users within the TTL window.
    pub const PLAYER_GAME_LOG_TTL: Duration = Duration::from_secs(600);

    /// Player bio and season totals.
    pub const PLAYER_DETAILS_TTL: Duration = Duration::from_secs(1800);

    /// League standings. Drives insights' streak and L10 fields.
    /// Matches the NHL's own post-game update cadence.
    pub const STANDINGS_TTL: Duration = Duration::from_secs(1800);

    /// Team roster. Trades and recalls are rare intraday events;
    /// 30 min bounds how long a roster change takes to propagate
    /// into the app.
    pub const ROSTER_TTL: Duration = Duration::from_secs(1800);

    /// NHL Edge telemetry (top skating speed, top shot speed, etc.).
    /// Season-aggregated; no minute-to-minute changes.
    pub const EDGE_TTL: Duration = Duration::from_secs(1800);

    /// `score/{date}` — used by the insights generator for the
    /// "last game result" sidebar. Irrelevant once the date settles.
    pub const SCORES_TTL: Duration = Duration::from_secs(120);
}

// ---------------------------------------------------------------------
// Scheduler (backend/src/utils/scheduler.rs)
// ---------------------------------------------------------------------

/// In-process cron scheduler. Jobs run once per tick in the single
/// backend process. The service is currently deployed as a single Fly
/// machine; if that changes to multi-instance, wrap these jobs in a
/// leader-election primitive before enabling the second replica —
/// otherwise every job runs N times per tick.
pub mod scheduler {
    use super::Duration;

    /// Morning rankings run. Computes yesterday's daily fantasy
    /// rankings from completed boxscores and writes them to the
    /// `daily_rankings` table. 09:00 UTC = 05:00 ET, which is after
    /// every West Coast game has finalized.
    ///
    /// Format: 6-field cron (`sec min hour dom mon dow`) per
    /// `tokio_cron_scheduler`. Earlier values of this file used
    /// `"0 9 * * * *"` which *parses* as 6-field but means
    /// "every hour at minute 9, seconds 0" — the job fired 24 times
    /// a day rather than once. The corrected form pins
    /// `hour = 9` explicitly.
    pub const MORNING_RANKINGS_CRON: &str = "0 0 9 * * *";

    /// Afternoon safety net. Re-runs the morning job at 15:00 UTC to
    /// cover any boxscores that the NHL published late.
    pub const AFTERNOON_RANKINGS_CRON: &str = "0 0 15 * * *";

    /// Daily prewarm. Ingests yesterday's playoff boxscores, refreshes
    /// the playoff roster cache, and generates the insights and
    /// race-odds payloads for the day so user requests hit Postgres
    /// rather than fan out to NHL. 10:00 UTC = 06:00 ET — before any
    /// game starts, which matters because the pre-game matchup block
    /// in `game-landing` is only present while the game is in FUT
    /// state.
    pub const DAILY_PREWARM_CRON: &str = "0 0 10 * * *";

    /// How long rows sit in the `response_cache` table before the
    /// morning rankings job prunes them.
    pub const CACHE_RETENTION: Duration = Duration::from_secs(7 * 24 * 3600);
}

// ---------------------------------------------------------------------
// HTTP paths that talk out of process
// ---------------------------------------------------------------------

/// Outbound HTTP timeouts for services other than NHL: Anthropic,
/// the Daily Faceoff scraper, and the Axum inbound server timeout.
pub mod http {
    use super::Duration;

    /// Outer Axum timeout applied by the middleware layer in
    /// `backend/src/api/mod.rs`. Caps a request's total server-side
    /// time including database reads and any out-of-process fallback.
    /// Must exceed [`super::nhl_client::REQUEST_TIMEOUT`] or the
    /// retry loop is cut off by the outer timeout.
    pub const AXUM_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

    /// Anthropic `/v1/messages` timeout. Covers both the insights
    /// haiku prompt and the pulse sonnet prompt. 30 s is comfortable
    /// for the largest prompt we send (full signals payload) plus the
    /// longest completion we request (3072 tokens).
    pub const CLAUDE_TIMEOUT: Duration = Duration::from_secs(30);

    /// Daily Faceoff headline scraper. The insights page renders
    /// without the news block if the scraper times out, so this is
    /// deliberately short — we prefer fast page loads to complete
    /// scraping.
    pub const HEADLINE_SCRAPER_TIMEOUT: Duration = Duration::from_secs(10);

    /// WebSocket keepalive ping cadence. Draft sessions can idle
    /// through intermediate proxies; 30 s is shorter than every proxy
    /// timeout we have seen in production.
    pub const WS_PING_INTERVAL: Duration = Duration::from_secs(30);
}

// ---------------------------------------------------------------------
// Live NHL mirror (forthcoming — constants reserved)
// ---------------------------------------------------------------------

/// Background-poller cadences for the NHL mirror pipeline described
/// in [docs/DATA-PIPELINE-REDESIGN.md](../../docs/DATA-PIPELINE-REDESIGN.md).
///
/// The pollers have not shipped yet; these constants are reserved so
/// the cadences are reviewable before the implementation lands. Once
/// the pollers are in place, the NHL client semaphore still guards
/// concurrency — these intervals govern how often the pollers wake,
/// not how many calls they make per wake.
pub mod live_mirror {
    use super::Duration;

    /// Live-poller period. How often to re-fetch boxscore and scores
    /// for every game whose state is LIVE or CRIT. 60 s matches the
    /// NHL CDN's own live-boxscore TTL — polling faster returns the
    /// same data.
    pub const LIVE_POLL_INTERVAL: Duration = Duration::from_secs(60);

    /// Meta-poller period. Refreshes schedule, standings, skater and
    /// goalie leaderboards, and playoff carousel into the mirror
    /// tables. 5 min is generous for data that moves a handful of
    /// times per day.
    pub const META_POLL_INTERVAL: Duration = Duration::from_secs(300);

    /// How often, measured in meta-poller ticks, to refresh team
    /// rosters. NHL rosters change via call-ups and trade-deadline
    /// moves — none of which happen during the playoffs, and all
    /// of which are tolerable at day-fresh resolution during the
    /// regular season. 288 ticks × 5 min = 24 h; the 10:00 UTC
    /// daily prewarm refreshes rosters explicitly anyway, so this
    /// is just belt-and-braces.
    pub const ROSTER_REFRESH_EVERY_N_META_TICKS: u32 = 288;

    /// How often, measured in meta-poller ticks, to refresh the
    /// "aggregates" — tomorrow's schedule, standings, skater and
    /// goalie leaderboards, and the playoff carousel. All change
    /// only when a game ends or a series resolves, not
    /// minute-to-minute. 6 ticks × 5 min = 30 min is generous.
    ///
    /// The long-term plan (noted in `docs/DATA-PIPELINE-REDESIGN.md`)
    /// is to make these event-driven: the live poller detects a
    /// `LIVE → OFF/FINAL` transition and triggers an aggregates
    /// refresh at that instant. Time-based polling here then
    /// becomes a safety net.
    pub const AGGREGATES_REFRESH_EVERY_N_META_TICKS: u32 = 6;

    /// Delay between process boot and the meta poller's first tick.
    /// `tokio::time::interval` fires immediately on the first poll,
    /// which at startup collides with the rankings-backfill NHL
    /// fan-out and produces a 429 cascade. 15 s gives the startup
    /// backfill enough room to finish.
    pub const META_POLL_STARTUP_DELAY: Duration = Duration::from_secs(15);

    /// Delay between process boot and the live poller's first tick.
    /// Must be greater than the time the meta poller takes to
    /// populate `nhl_games` for today, otherwise the live poller
    /// wakes to an empty table and has no games to poll for one
    /// whole interval.
    pub const LIVE_POLL_STARTUP_DELAY: Duration = Duration::from_secs(45);

    /// Sleep between consecutive roster fetches inside a single meta
    /// tick. The meta poller walks all 32 NHL rosters in sequence;
    /// back-to-back requests at full speed trip the NHL per-IP rate
    /// limit (~20 req/s observed) and the last third of teams gets
    /// served exhausted-budget 429s. 250 ms inflates one roster-
    /// refresh tick from ~6 s to ~14 s but gives every team a clean
    /// fetch.
    pub const ROSTER_FETCH_DELAY: Duration = Duration::from_millis(250);
}
