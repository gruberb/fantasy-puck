# HTTP API

Every endpoint the backend serves, grouped by concern. Routes are defined in one place: [`backend/src/api/routes.rs`](../backend/src/api/routes.rs). This document is derived from that file; if something here disagrees with `routes.rs`, the routes file wins.

## Response envelope

Successful responses have two fields ([`api/response.rs:4-8`](../backend/src/api/response.rs)):

```json
{
  "success": true,
  "data": { ... }
}
```

Error responses have two fields ([`error.rs:49-85`](../backend/src/error.rs)):

```json
{
  "success": false,
  "error": "Resource not found"
}
```

Mapping from internal `Error` variants to HTTP status codes:

| Error variant | Status | Body `error` |
| --- | --- | --- |
| `Database(_)` | 500 | `"Database error occurred"` (the real error is logged, not surfaced) |
| `NhlApi(_)` | 502 | `"External service error"` |
| `NotFound(msg)` | 404 | `msg` |
| `Validation(msg)` | 400 | `msg` |
| `Unauthorized(msg)` | 401 | `msg` |
| `Forbidden(msg)` | 403 | `msg` |
| `Conflict(msg)` | 409 | `msg` |
| `Internal(_)` | 500 | `"Internal server error"` (real error logged) |

## Authentication

JWT-based. The token goes in the `Authorization` header: `Bearer <jwt>`.

- **`AuthUser` extractor** ([`auth/middleware.rs:21-46`](../backend/src/auth/middleware.rs)) - required on most endpoints. Validates the token, returns 401 if missing or invalid.
- **`OptionalAuth` extractor** ([`auth/middleware.rs:49-79`](../backend/src/auth/middleware.rs)) - accepts a token but does not require one. Invalid tokens still 401.
- **Admin endpoints** additionally check `auth.is_admin` inside the handler; 403 if unset.

Claim shape: `{ sub: user_id, email, is_admin }`. The secret is `config.jwt_secret`.

---

## Endpoint index

### Health

| Method | Path | Handler | Auth | Notes |
| --- | --- | --- | --- | --- |
| GET | `/health/live` | inline (`\|\| async { StatusCode::OK }`) at [`routes.rs:50`](../backend/src/api/routes.rs) | None | Liveness probe, no DB check |
| GET | `/health/ready` | `health_ready` at [`routes.rs:283-290`](../backend/src/api/routes.rs) | None | Pings DB; 503 on failure |

### Auth

All handlers in [`backend/src/api/handlers/auth.rs`](../backend/src/api/handlers/auth.rs).

| Method | Path | Handler | Auth | Returns |
| --- | --- | --- | --- | --- |
| POST | `/api/auth/login` | `login` | None | `AuthResponse { token, user, profile }` |
| POST | `/api/auth/register` | `register` | None | Same as login |
| GET | `/api/auth/me` | `get_me` | Required | `MeResponse { user, profile }` |
| PUT | `/api/auth/profile` | `update_profile` | Required | Updated profile |
| DELETE | `/api/auth/account` | `delete_account` | Required | Cascades via `delete_user_account()` stored proc |
| GET | `/api/auth/memberships` | `get_memberships` | Required | `Vec<MembershipRow>` - leagues and teams the user belongs to |

### Leagues

Handlers in [`handlers/leagues.rs`](../backend/src/api/handlers/leagues.rs).

| Method | Path | Handler | Auth | Returns / data source |
| --- | --- | --- | --- | --- |
| GET | `/api/leagues` | `list_leagues` | Optional | `Vec<League>` from `leagues`; filter by `visibility` query |
| POST | `/api/leagues` | `create_league` | Required | `LeagueRow`; creator recorded in `created_by` |
| DELETE | `/api/leagues/{league_id}` | `delete_league` | Required (owner) | Cascades to teams / members / picks |
| GET | `/api/leagues/{league_id}/members` | `get_league_members` | Required | `Vec<LeagueMemberRow>` joined with profiles and teams |
| POST | `/api/leagues/{league_id}/join` | `join_league` | Required | Inserts `fantasy_teams` + `league_members` row |
| DELETE | `/api/leagues/{league_id}/members/{member_id}` | `remove_member` | Required | Removes a member from a league |

### Draft

Handlers in [`handlers/draft.rs`](../backend/src/api/handlers/draft.rs). The full draft lifecycle is covered in [`08-draft.md`](./08-draft.md); this table is the route listing.

| Method | Path | Handler | Auth | Broadcasts on WS? |
| --- | --- | --- | --- | --- |
| GET | `/api/leagues/{league_id}/draft` | `get_draft_by_league` | Required | - |
| POST | `/api/leagues/{league_id}/draft` | `create_draft_session` | Required | - |
| POST | `/api/leagues/{league_id}/draft/randomize-order` | `randomize_order` | Required | - |
| GET | `/api/draft/{draft_id}` | `get_draft_state` | Required | - |
| DELETE | `/api/draft/{draft_id}` | `delete_draft` | Required | - |
| POST | `/api/draft/{draft_id}/populate` | `populate_player_pool` | Required | `PlayerPoolUpdated` |
| POST | `/api/draft/{draft_id}/start` | `start_draft` | Required | `SessionUpdated` |
| POST | `/api/draft/{draft_id}/pause` | `pause_draft` | Required | `SessionUpdated` |
| POST | `/api/draft/{draft_id}/resume` | `resume_draft` | Required | `SessionUpdated` |
| POST | `/api/draft/{draft_id}/pick` | `make_pick` | Required | `PickMade` + `SessionUpdated` |
| POST | `/api/draft/{draft_id}/finalize` | `finalize_draft` | Required | `SessionUpdated` |
| POST | `/api/draft/{draft_id}/complete` | `complete_draft` | Required | `SessionUpdated` |
| GET | `/api/draft/{draft_id}/sleepers` | `get_eligible_sleepers` | Required | - |
| GET | `/api/draft/{draft_id}/sleeper-picks` | `get_sleeper_picks` | Required | - |
| POST | `/api/draft/{draft_id}/sleeper/start` | `start_sleeper_round` | Required | `SessionUpdated` |
| POST | `/api/draft/{draft_id}/sleeper/pick` | `make_sleeper_pick` | Required | `SleeperUpdated` |

### Fantasy teams and players

Handlers in [`handlers/teams.rs`](../backend/src/api/handlers/teams.rs) and [`handlers/players.rs`](../backend/src/api/handlers/players.rs).

| Method | Path | Handler | Auth | Data source |
| --- | --- | --- | --- | --- |
| GET | `/api/fantasy/teams` | `list_teams` | Required | `db.get_all_teams(league_id)` |
| GET | `/api/fantasy/teams/{id}` | `get_team` | Required | `fantasy_teams` + `fantasy_players`, joined with the NHL skater-stats leaderboard. The rich per-player breakdown + descriptive diagnosis lives on `/api/pulse` instead. |
| PUT | `/api/fantasy/teams/{id}` | `update_team_name` | Required | `UPDATE fantasy_teams` |
| POST | `/api/fantasy/teams/{id}/players` | `add_player_to_team` | Required | `INSERT fantasy_players` |
| DELETE | `/api/fantasy/players/{player_id}` | `remove_player` | Required | `DELETE fantasy_players` |
| GET | `/api/fantasy/players` | `get_players_per_team` | Required | Groups `fantasy_players` by `nhl_team` |
| GET | `/api/fantasy/team-bets` | `get_team_bets` | Required | Count of rostered players per NHL team, per fantasy team |
| GET | `/api/fantasy/team-stats` | `get_team_stats` | Required | Aggregated team statistics |
| GET | `/api/fantasy/league-stats` | `get_league_stats` | Required | League-wide roster concentration + top-10 rostered skaters by playoff points (drives the /stats League Stats section) |

### Rankings

Handlers in [`handlers/rankings.rs`](../backend/src/api/handlers/rankings.rs).

| Method | Path | Handler | Auth | Data source |
| --- | --- | --- | --- | --- |
| GET | `/api/fantasy/rankings` | `get_rankings` | Required | Sums `nhl_player_game_stats` per fantasy team for the active season |
| GET | `/api/fantasy/rankings/daily` | `get_daily_rankings` | Required | `nhl_player_game_stats` for the requested date; falls back to `daily_rankings` |
| GET | `/api/fantasy/rankings/playoffs` | `get_playoff_rankings` | Required | `playoff_skater_game_stats` summed per fantasy team |

### Sleepers

Handlers in [`handlers/sleepers.rs`](../backend/src/api/handlers/sleepers.rs).

| Method | Path | Handler | Auth | Data source |
| --- | --- | --- | --- | --- |
| GET | `/api/fantasy/sleepers` | `get_sleepers` | Required | `fantasy_sleepers` for league |
| DELETE | `/api/fantasy/sleepers/{sleeper_id}` | `remove_sleeper` | Required | `DELETE fantasy_sleepers` |

### NHL data (mirror reads)

Handlers in [`handlers/games.rs`](../backend/src/api/handlers/games.rs), [`handlers/stats.rs`](../backend/src/api/handlers/stats.rs), [`handlers/playoffs.rs`](../backend/src/api/handlers/playoffs.rs), [`handlers/nhl_rosters.rs`](../backend/src/api/handlers/nhl_rosters.rs).

| Method | Path | Handler | Auth | Data source |
| --- | --- | --- | --- | --- |
| GET | `/api/nhl/games` | `list_games` | Optional | `nhl_games` + `nhl_player_game_stats`; extended shape when `league_id` + `detail=extended` is passed |
| GET | `/api/nhl/match-day` | `get_match_day` | Required | `nhl_games` for today (+ yesterday if early morning) + fantasy overlays; cached at `match_day:{date}` |
| GET | `/api/nhl/skaters/top` | `get_top_skaters` | Optional | Playoffs: `nhl_player_game_stats` aggregate with G/A/P, PIM, +/-, and TOI/gm; regular season: NHL API fallback wrapped in `response_cache`. Optional `league_id` adds fantasy-team ownership tags. |
| GET | `/api/nhl/roster/{team}` | `get_team_roster` | Optional | `nhl_team_rosters` (JSONB roster) |
| GET | `/api/nhl/playoffs` | `get_playoff_info` | Optional | `nhl_playoff_bracket` |

### Insights

Handler in [`handlers/insights.rs`](../backend/src/api/handlers/insights.rs).

| Method | Path | Handler | Auth | Data source |
| --- | --- | --- | --- | --- |
| GET | `/api/insights` | `get_insights` | Optional | `response_cache` (keyed per league) with miss-through to mirror reads + Claude narrative + Daily Faceoff headline scraper |

The response includes a generated narrative and a set of signals: hot players, cold players, today's slate, series projections, and a **Last Night** recap (games that finalised on the previous hockey-date with their final score, post-game series state, and top scorers). The narrative object has four fields — `todays_watch`, `game_narratives[]`, `hot_players`, `bracket`, and `last_night` (a markdown-ish string with `### Subheading` per game in a Daily Faceoff voice). Narratives are cached per-day; invalidation is controlled by the daily prewarm cron, which overwrites the `insights:*` rows at 10:00 UTC. To force a regeneration mid-day, hit `GET /api/admin/cache/invalidate?scope=today`.

### Pulse

Handler in [`handlers/pulse.rs`](../backend/src/api/handlers/pulse.rs).

| Method | Path | Handler | Auth | Data source |
| --- | --- | --- | --- | --- |
| GET | `/api/pulse?league_id=...` | `get_pulse` | Required | `v_daily_fantasy_totals` (live), `nhl_games`, `nhl_playoff_bracket`, `nhl_player_game_stats` rollup, yesterday's mirror recap, cached race-odds payload, plus the per-caller `MyTeamDiagnosis` bundle cached at `team_diagnosis:{league}:{team}:{season}:{gt}:{date}:bundle:v1` (which itself nests the Claude narrative cached at `…:v2`) |

`PulseResponse` carries: the `leagueBoard` (with live `pointsToday` from `v_daily_fantasy_totals`, consumed by the dashboard's Live Rankings section — the Pulse page itself no longer renders this list), a per-team `seriesForecast`, the caller's `myGamesTonight`, a flat `gamesToday` list of today's NHL matchups, and an `nhlTeamCupOdds` map lifted from the cached race-odds payload.

Two additional blocks drive the Pulse page directly:

- **`myTeamDiagnosis`** — full per-player breakdown (G/A/P/SOG/PIM/+/-/HIT/TOI, projected PPG, letter grade, bucket, remaining-points impact, last-5 games) plus a descriptive `diagnosis.narrativeMarkdown`. `diagnosis.yesterday` is built from `nhl_games` + `nhl_player_game_stats` for the previous hockey date, with a current top-3 league fallback when nobody relevant appeared. The narrative has four `### Heading` sections — Yesterday / Where You Stand / Player-by-Player / What to Expect — written in a descriptive-not-prescriptive voice since the roster is locked for the playoffs.
- **`leagueOutlook`** — leader + points distribution across all teams, plus the top-3 projected finishers from the Monte Carlo with each team's largest NHL stack and its cup-win probability.

Both blocks share one composition helper ([`handlers/team_breakdown::compose_team_breakdown`](../backend/src/api/handlers/team_breakdown.rs)). The daily 10:00 UTC prewarm job + the on-demand `GET /api/admin/prewarm` endpoint call `pulse::resolve_my_team_diagnosis` per (league × team) so the bundle cache is filled before any user lands on Pulse — the first request becomes a single SELECT.

**Bundle cache**: `team_diagnosis:{league_id}:{team_id}:{season}:{gt}:{date}:bundle:v1`. Holds the full `MyTeamDiagnosis` payload (per-player breakdown + `diagnosis.narrativeMarkdown`). Survives mid-evening game-end transitions intentionally — wiping it would force a synchronous Claude regen on the next Pulse load. Ages out on the date roll; rebuilt by the next morning's prewarm.

**Narrative cache**: `team_diagnosis:{league_id}:{team_id}:{season}:{gt}:{date}:v2`. The Claude-generated text only. Invalidated by the live poller on `LIVE|CRIT → OFF|FINAL` transitions for games that any rostered player was in, so the next prewarm regenerates the narrative with the final score in view and folds it into the new bundle.

### Race odds

Handler in [`handlers/race_odds.rs`](../backend/src/api/handlers/race_odds.rs).

| Method | Path | Handler | Auth | Data source |
| --- | --- | --- | --- | --- |
| GET | `/api/race-odds?league_id=...&mode=...` | `get_race_odds` | Optional | `response_cache` with miss-through to the Monte Carlo simulator in `domain::prediction::race_sim` |

Modes: League (league-scoped race) or Champion (global Fantasy Champion board). See [`05-prediction-engine.md`](./05-prediction-engine.md) for the simulation detail.

### Admin

All admin handlers check `auth.is_admin` and return 403 if unset. Handlers in [`handlers/admin.rs`](../backend/src/api/handlers/admin.rs). Full behavior described in [`07-background-jobs.md`](./07-background-jobs.md). Every endpoint below is also surfaced as a one-click panel in the admin dashboard at `/admin` (admin-only route), with inline response rendering — see `frontend/src/pages/AdminDashboardPage.tsx`.

| Method | Path | Handler | Returns |
| --- | --- | --- | --- |
| GET | `/api/admin/process-rankings/{date}` | `process_rankings` | String confirmation |
| GET | `/api/admin/cache/invalidate?scope=(all\|today\|{date})` | `invalidate_cache` | String confirmation |
| GET | `/api/admin/backfill-historical?from=&to=` | `backfill_historical_playoffs` | Row-count summary |
| GET | `/api/admin/rebackfill-carousel?season=` | `rebackfill_carousel` | Summary |
| GET | `/api/admin/calibrate?season=&...` | `calibrate` | `CalibrationReport` |
| GET | `/api/admin/calibrate-sweep?points_scale=&shrinkage=&k_factor=&home_ice_elo=&trials=` | `calibrate_sweep_handler` | `SweepReport` (capped at 200 cells) |
| GET | `/api/admin/prewarm` | `prewarm_cache` | `"Pre-warm started in background..."` (spawns a background task) |
| GET | `/api/admin/rehydrate` | `rehydrate_mirror` | `RehydrateSummary { games_upserted, boxscore_player_rows, errors, ... }` |

### WebSocket

| Method | Path | Handler | Auth |
| --- | --- | --- | --- |
| GET | `/ws/draft/{session_id}` | `ws::handler::ws_draft` at [`backend/src/ws/handler.rs`](../backend/src/ws/handler.rs) | Optional (token via query string) |

See [`08-draft.md`](./08-draft.md) for the message types and reconnect behavior.

---

## Cache keys

All written to `response_cache` by handlers. All read from the same table. Invalidation is either lifecycle-driven (live poller on game end) or operator-driven (`/api/admin/cache/invalidate`).

| Cache key | Written by | Invalidated by |
| --- | --- | --- |
| `match_day:{date}` | `handlers/games.rs` (`get_match_day`) | Admin invalidate, scope=`{date}` |
| `insights:...` | `handlers/insights.rs` | Daily prewarm overwrites at 10:00 UTC; admin invalidate |
| `race_odds:...` | `handlers/race_odds.rs` | Same pattern as insights |
| `team_diagnosis:{league_id}:{team_id}:{season}:{gt}:{date}:v2` | `handlers/team_breakdown.rs` (Claude narrative) | Live poller's `invalidate_by_like("team_diagnosis:{league}:%:v2")` on `LIVE \| CRIT → OFF \| FINAL` for games involving any rostered player |
| `team_diagnosis:{league_id}:{team_id}:{season}:{gt}:{date}:bundle:v1` | `handlers/pulse.rs` (`resolve_my_team_diagnosis`) | Daily 10:00 UTC prewarm overwrite; admin invalidate. Not wiped on game-end — see `06-business-logic.md`. |

## Query params worth knowing

- `league_id` - required on most fantasy and league-scoped endpoints. UUID.
- `visibility` on `GET /api/leagues` - `public` or `private`.
- `date` on `GET /api/fantasy/rankings/daily` and `/api/admin/process-rankings/{date}` - `YYYY-MM-DD`.
- `scope` on `/api/admin/cache/invalidate` - `all`, `today`, or a specific `YYYY-MM-DD`.
- `detail=extended` on `GET /api/nhl/games` - includes per-team fantasy overlays (form, totals) when combined with `league_id`.
- `mode` on `GET /api/race-odds` - `league` (default) or `champion`.
