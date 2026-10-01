# Backend architecture

A map of the backend: what lives where, how the pieces fit, and what happens between a request arriving at the port and a response going back.

## Stack

- Rust with Tokio and Axum ([`backend/Cargo.toml`](../backend/Cargo.toml)).
- PostgreSQL via `sqlx` (Supabase in production).
- The NHL web API (`api-web.nhle.com`) as the source of truth for hockey data.
- OpenRouter (`openrouter.ai`) for narrative text (optional; a null adapter is wired if the key is absent).

## Layer split

The crate is divided into three layers, documented in [`backend/src/lib.rs`](../backend/src/lib.rs):

| Layer | Location | What it owns |
| --- | --- | --- |
| `domain` | `backend/src/domain/` | Pure business logic. No Axum, no `sqlx`, no `reqwest`. Exposes `ports::*` traits that `infra` implements. |
| `infra` | `backend/src/infra/` | Adapters. Postgres (`infra::db`), NHL API (`infra::nhl`), OpenRouter (`infra::prediction`), scheduled jobs (`infra::jobs`). |
| `api` | `backend/src/api/` | Axum handlers, DTOs, routes, extractors, middleware. |

`main.rs` is the composition root: it constructs the concrete adapters, wires them into `api::AppState`, and spawns background jobs.

## Module tree

```
backend/src/
├── main.rs             # Composition root (boot sequence)
├── lib.rs              # Re-exports + module declarations
├── config.rs           # Typed env-var loading; panics early if required vars missing
├── tuning.rs           # All magic numbers (timeouts, TTLs, cron expressions, poller cadences)
├── error.rs            # Crate-wide Error / Result
│
├── api/
│   ├── mod.rs          # Axum server bootstrap, middleware stack, season OnceLock
│   ├── routes.rs       # Route table + AppState struct
│   ├── response.rs     # JSON envelope utilities
│   ├── handlers/       # One file per concern (auth, leagues, draft, pulse, insights, ...)
│   └── dtos/           # Request/response types
│
├── auth/
│   ├── middleware.rs   # AuthUser / OptionalAuth extractors (Bearer token)
│   ├── jwt.rs          # Token issue / validate
│   └── password.rs     # argon2 hashing / verify
│
├── domain/
│   ├── models/         # Entity types (db, fantasy, nhl)
│   ├── ports/          # Trait definitions (PredictionService)
│   ├── services/       # Stateless logic (rankings)
│   ├── time.rs         # ET "hockey date" helpers (the NHL keys games by ET date)
│   └── prediction/     # Forward model: player projection, Elo, race sim, calibration
│
├── infra/
│   ├── db/             # Postgres readers/writers, one module per concern
│   ├── nhl/            # NHL HTTP client + constants
│   ├── jobs/           # Background pollers, scheduler, one-shot seeds
│   ├── prediction/     # LLM narrator + null fallback, OpenRouter client
│   └── calibrate.rs    # K-factor calibration against historical playoff results
│
└── ws/
    ├── handler.rs      # /ws/draft/:session_id upgrade handler
    └── draft_hub.rs    # In-process broadcast hub for draft events
```

## Boot sequence

From [`backend/src/main.rs`](../backend/src/main.rs):

1. `dotenv().ok()` loads `.env` if present.
2. `Config::from_env()` eagerly loads and validates env vars; panics on missing required values (see [`config.rs`](../backend/src/config.rs)). This includes `OPENROUTER_API_KEY` and `OPENROUTER_MODEL` (default `anthropic/claude-sonnet-5.5`).
3. Tracing subscriber is initialized (JSON or pretty depending on `LOG_JSON`).
4. `api::init_season_config` populates four `OnceLock` cells used by handlers: `season()`, `game_type()`, `playoff_start()`, `season_end()` (see [`api/mod.rs`](../backend/src/api/mod.rs)); `is_playoffs()` derives from `game_type()`.
5. `NhlClient::new()` and its cache-cleanup task spawn.
6. `FantasyDb::new()` opens a Postgres pool; `sqlx::migrate!` runs pending migrations idempotently on every boot.
7. The prediction adapter is composed: one shared `OpenRouterClient` if `OPENROUTER_API_KEY` is set, wrapped in `LlmNarrator`; `NullNarrator` otherwise. A single `AppState` (db, NHL client, config, draft hub, prediction adapter, LLM client) is built here and shared by the router, the scheduler, and the admin prewarm.
8. `init_rankings_scheduler(state)` registers four crons (see [`07-background-jobs.md`](./07-background-jobs.md)).
9. Background tasks are spawned:
   - Historical-skater CSV seed, if `historical_playoff_skater_totals` is empty.
   - Rankings and playoff-game-stats backfill, if those tables are empty. A failed emptiness check skips the backfill rather than triggering it.
   - Meta and live pollers.
   - Auto-seed: after `LIVE_POLL_STARTUP_DELAY`, if `nhl_player_game_stats` is empty, run `rehydrate`.
10. `api::run_server(state)` binds and accepts on the configured port.
11. On SIGTERM or Ctrl+C, graceful shutdown fires and `poller_cancel.cancel()` unwinds the pollers.

## `AppState`

Every handler receives `Arc<AppState>` as `State<Arc<AppState>>`. Defined in [`api/routes.rs:17-28`](../backend/src/api/routes.rs):

```rust
pub struct AppState {
    pub db: FantasyDb,
    pub nhl_client: NhlClient,
    pub config: Arc<Config>,
    pub draft_hub: DraftHub,
    pub prediction: Arc<dyn PredictionService>,
}
```

- `db` - Postgres pool (max 5 connections, statement cache disabled for PgBouncer).
- `nhl_client` - NHL HTTP adapter with in-memory cache and semaphore rate limit.
- `config` - read-only.
- `draft_hub` - in-process broadcast hub for WebSocket draft events.
- `prediction` - text-generation trait object; production is `LlmNarrator`, fallback is `NullNarrator`.

## Middleware stack

Middleware is applied in [`api/mod.rs:65-71`](../backend/src/api/mod.rs). Layers wrap in reverse order; the last `.layer()` is the outermost:

```
request in  →  CompressionLayer
             →  TimeoutLayer          (30 s, tuning::http::AXUM_REQUEST_TIMEOUT)
             →  RequestBodyLimitLayer (1 MB)
             →  CorsLayer
             →  Router
response out ←
```

CORS is open (`Any`) when `CORS_ORIGINS` is empty; otherwise the env var is comma-split and each origin parsed (`mod.rs:56-63`). Allowed methods: `GET, POST, PUT, DELETE, OPTIONS`. Allowed headers: `CONTENT_TYPE`, `AUTHORIZATION`.

## Request flow

```
          ┌─────────────┐
   HTTP → │  middleware │  (CORS, body limit, timeout, compression)
          └──────┬──────┘
                 ▼
          ┌─────────────┐
          │   handler   │  (extracts State<AppState>, Path<..>, Json<..>)
          └──────┬──────┘
                 ▼
         ┌───────┴───────┬─────────────┬────────────────┐
         ▼               ▼             ▼                ▼
    ┌─────────┐   ┌─────────────┐  ┌────────┐   ┌───────────────┐
    │  db::*  │   │ nhl_mirror  │  │  cache │   │   prediction  │
    │ readers │   │  readers    │  │        │   │    adapter    │
    └────┬────┘   └──────┬──────┘  └────┬───┘   └───────┬───────┘
         ▼               ▼              ▼               ▼
      Postgres       Postgres       Postgres       OpenRouter
```

Almost every user-facing read goes through `db` (fantasy state) or `nhl_mirror` (NHL facts). Handlers are discouraged from calling the NHL API directly on the request path; the mirror is there so hot reads are one SELECT, not a fan-out to `api-web.nhle.com`. The few endpoints that still call the NHL API on a cache miss (for example the regular-season skater leaderboard fallback in `handlers/stats.rs`) are wrapped in `response_cache` reads first.

For the data flow during a live NHL game, see [`06-business-logic.md`](./06-business-logic.md). For the cron jobs and pollers that keep the mirror warm, see [`07-background-jobs.md`](./07-background-jobs.md).

## Binary entry point

One binary, one default subcommand:

```
fantasy_hockey [serve] [--port 3000]
```

`serve` is the default and is currently the only command ([`main.rs`](../backend/src/main.rs)). The `--port` flag overrides `PORT` from the environment.

## Configuration

Two sources of configuration, with different change cadences:

| Source | File | What's there | When to edit |
| --- | --- | --- | --- |
| Environment | `backend/.env`, deploy config | Secrets, DB URL, season/game-type, CORS origins, port | Per-environment |
| Compiled constants | [`backend/src/tuning.rs`](../backend/src/tuning.rs) | Timeouts, TTLs, cron expressions, poller cadences, retry counts, calibration constants | Requires redeploy |

The four fields in `Config` that describe the current NHL season (`nhl_season`, `nhl_game_type`, `nhl_playoff_start`, `nhl_season_end`) are surfaced via `OnceLock` accessors so handlers can read them without threading `Config` through every call. Switching between regular season and playoffs is documented in the top-level [`CLAUDE.md`](../CLAUDE.md) under "Switching season / game type".

## Error handling

Crate-wide `Error` type in [`backend/src/error.rs`](../backend/src/error.rs) with automatic conversions from `sqlx::Error`, `reqwest::Error`, and others. Handlers return `Result<T>` and rely on `IntoResponse` to serialize errors into the `{ success: false, error: { code, message } }` envelope. See [`03-api.md`](./03-api.md) for the response shape.

## What this architecture does not include

- No service mesh or inter-service RPC; this is a single process that talks to Postgres and a handful of external HTTP APIs.
- No event bus. Fan-out to clients during drafts uses an in-process Tokio broadcast channel. Multi-replica coordination for pollers uses Postgres advisory locks, not a queue.
- No ORM in the handler layer. Every query is written by hand in `infra::db::*` modules; handlers call those functions, not `sqlx::query` directly.
