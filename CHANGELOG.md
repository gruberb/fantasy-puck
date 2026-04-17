# Changelog

All notable changes to Fantasy Puck are documented here.

## Unreleased

## v1.7.2 — 2026-04-17

### Changed
- **Insights ownership pills now you-only** — v1.7.1's +N MORE toggle helped on desktop but still looked horrible on mobile in `StanleyCupOdds`, where the Team column is ~80px wide and each chip stacked vertically. In 15-team leagues the cross-league ownership list was noise anyway — the signal you scan for during a game is "do I have skin in this?". `RosteredChips` now renders a single `YOU: {count}` pill when the caller owns players on that NHL team, and nothing otherwise. Matching is done via `useLeague()` + active-league membership, so no backend changes.
- **StanleyCupOdds mobile grid** — dropped the phantom 6th grid column on mobile (the "Final" column was hidden but the track still reserved 3rem, truncating team names to "AVA…"). Mobile now has 5 tracks matching the 5 visible cells; desktop keeps the full 6.

## v1.7.1 — 2026-04-17

### Changed
- **Insights rostered-by chips collapse** — with 15-team leagues, the fantasy-ownership chip strip on `StanleyCupOdds` and `PlayoffBracketTree` wrapped 3–4 lines on desktop and stacked vertically on mobile, drowning the data in yellow pills. Now shows the top 3 teams by count inline with a `+N MORE` toggle that expands in place. Extracted a shared `RosteredChips` component (was duplicated across both files). Toggle uses an inverting hover/active state, `touch-manipulation` to kill tap delay, and `aria-expanded` for screen readers.

## v1.7.0 — 2026-04-17

Headline change: Pulse is now the personal/league-race page (your standing, your projections, your rivalry, your NHL stakes) and Insights is the NHL-generic page (today's games, hot/cold skaters, bracket, Stanley Cup odds). A new Monte Carlo engine (`race_sim`) underpins every projection on both pages, re-running every morning at 10am UTC.

### Added
- **`backend/src/utils/race_sim.rs`** — team-correlated Monte Carlo, 5,000 bracket trials per run. Per-game win probability = `sigmoid(k · (rating_top − rating_bottom))` with `k = 0.010` (calibrated against HockeyStats.com round-1 reference odds). Outputs per-fantasy-team `projected_final_mean / p10 / p90 / win_prob / top3_prob`, exact pairwise `head_to_head[opponent_id]` from per-trial score comparisons, per-NHL-team `advance_round1_prob / conference_finals_prob / cup_finals_prob / cup_win_prob / expected_games`, and per-player `projected_final_mean / p10 / p90`. Deterministic via `simulate_with_seed` in tests.
- **`/api/race-odds`** (new endpoint) — League mode returns fantasy-team odds + rivalry card + NHL cup odds. Champion mode returns the top-20 skater leaderboard by projected playoff fantasy points for the no-league/global Insights view. Cached per `(league_id, season, game_type, date)` and pre-warmed at 10am UTC alongside Insights.
- **`backend/src/utils/team_ratings.rs`** — shared blended team-strength rating: `0.7 × season_points + 0.3 × (L10_points_per_game × 82)`. Hot teams rise a few points above their season mark, cold teams drop. Used by both the race-odds engine and the Insights bracket enrichment.
- **Race Odds section on Pulse** — horizontal per-team win-probability bars + a columnar `LeagueRaceTable` (rank · team · current pts · projected · likely range · win% · "you beat X%"). Top-3 column auto-hides in leagues of ≤ 3 teams.
- **Rivalry / Head-to-Head card** — divergent bar (yellow = you, slate = rival) showing `P(you finish ahead of closest rival)` computed from exact MC pairwise samples. Hidden in 2-team leagues (the race board covers the same ground). Compact variant lives on Pulse as a hero line; full card on Insights for ≥3-team leagues.
- **My Stakes section on Pulse** — every NHL team the caller rosters, sorted by impact (`player_count × expected_games`). Per row: series context, `win R1 / reach Final / win Cup`, `expected_games`, linked player chips.
- **Stanley Cup Odds table on Insights** — championship-focused ranked list of every still-alive NHL playoff team. Columns: team · series · `win R1` · `reach Final` · **`win Cup`** · `expected games` · fantasy ownership pills. Methodology footnote ("Monte Carlo · 5,000 trials · team strength from regular-season standings points · current series state as the starting condition · re-run every morning · calibrated against HockeyStats.com round-1 reference odds within ~3pp") so users understand the inputs and the limitations.
- **PlayoffBracketTree on Insights** — replaces the old 16-card per-team grid. Per matchup: two team rows with score, strength-tag (Favored / Even / Underdog), blended team strength shown as `STRENGTH {n}` with an `ⓘ` tooltip explaining the blend, fantasy-team ownership pills, historical % to advance.
- **Pulse Claude narrative** — Sonnet 4.6, 1,500 max tokens, personal second-person voice, strict no-generic-AI-voice prompt (banned phrases: "dive in", "unleash", "game-changer", "buckle up", bulleted listicles). Hero position on Pulse, keyed by the caller's team so each user gets their own narrative. Falls through gracefully to no-narrative when `ANTHROPIC_API_KEY` is unset.
- **Fantasy Champion leaderboard** — global/no-league Insights view ranks the top 20 NHL skaters by `PPG × E[games_remaining]` from the same MC sweep. Useful primer for unauthenticated visitors.
- **Player headshots & NHL profile links** — every player name on Pulse's Series Rosters (regular + condensed), Insights Hot+Cold cards, and Pulse MyStakes links out to `nhl.com/player/{id}` in a new tab. Shared helper: `nhlPlayerProfileUrl`.
- **Analytical color tokens** — formalised palette in `index.css`: `--color-you` (warm yellow identity), `--color-rival` (cool slate, replaces the red that used to imply "danger" in rivalry views), `--color-ink-muted` (secondary text, same hex as rival by design). Rival is never red — red is reserved for elimination/error states only.
- **Hot/Cold regular-season fallback** — pre-playoffs, Hot/Cold sources from regular-season leader data instead of empty playoff stats. Cards render with "N season pts" instead of "N playoff pts"; an italic disclaimer sits above the section; Claude is prompted to use "regular-season points" in its narrative. Driven by a new `hotColdIsRegularSeason` flag on `InsightsSignals`.
- **Feature folder `features/race-odds/`** — new folder with `types.ts`, `hooks/use-race-odds.ts`, and six components (`RaceOddsSection`, `LeagueRaceBoard`, `LeagueRaceTable`, `FantasyChampionBoard`, `RivalryCard`, `MyStakes`). No cross-feature imports, no barrel re-exports (per Bulletproof React).

### Changed
- **`DEFAULT_K_FACTOR: 0.03 → 0.010`** — calibrated against HockeyStats.com round-1 reference odds. The prior value over-concentrated Cup probability on the top standings seed (Colorado came out at ~39% Cup where HockeyStats had them at ~13%). At `k = 0.010` our Cup distributions land within ~3pp of the reference.
- **Exact pairwise head-to-head** — `compute_rivalry` now reads directly from `TeamOdds.head_to_head[opponent_id]` (MC-counted per-trial comparisons) instead of a Welch-style normal approximation over `(p10, p90)`. Resolves a visible inconsistency where Insights showed 12% win-race while Pulse showed 10% finish-ahead for the same 2-team league; both surfaces now report identical numbers.
- **Pulse layout** — new top-down order: Claude narrative → head-to-head hero line → Race Odds → My Stakes → Series Rosters (renamed from "Series Forecast" — the old name implied prediction where the box actually shows ownership × series state) → Today's Pulse → My Players In Action → League Live Board.
- **Insights layout** — What to Watch Today → Hot + Cold → Bracket → Stanley Cup Odds → Fantasy Champion (global only) → Around the League.
- **Hot + Cold cards** — stacked rows (not side-by-side columns) so cards don't clip at a half-column width. Each card: `flex-col min-h-[230px]` with `mt-auto` footer block. Optional edge-data and fantasy-team-roster rows reserve their space even when empty so cards line up across the row. Stats grid now includes `{playoff_points} playoff pts` secondary line (or `season pts` during the pre-playoff fallback) to match what Claude's narrative references.
- **Series Rosters (Pulse) off-day condensation** — when every cell is a tied 0-0 series the 20-card grid collapses to a per-NHL-team row with linked avatar chips. Counting logic now separates `players_tied` from `players_trailing` (a tied series isn't losing).
- **`FantasyTeamForecast.players_tied`** — new field on the Pulse DTO; the old backend lumped tied into trailing and rendered "10 players — 10 trailing" even when every series was 0-0. Pre-bracket edge-case: headline collapses to "awaiting puck drop".
- **`PlayerForecastCell.nhl_id`** — new field so the frontend can build NHL profile links.
- **`HotPlayerSignal.nhl_id`** — ditto for Hot/Cold cards.
- **Scheduler pre-warm** — the 10am UTC job now warms both insights and race-odds caches for every league + the global view.
- **Claude Insights prompt** — rewritten to banish generic-AI voice, made NHL-centric (league-race framing lives on Pulse now), reduced to four content fields (`todays_watch`, `game_narratives`, `hot_players`, `bracket`). Respects the `hot_cold_is_regular_season` flag.
- **Bracket / Stanley Cup labels** — "RS pts" → "STRENGTH {n} ⓘ" with a tooltip explaining the blended rating so the number isn't mistaken for fantasy or playoff points.

### Fixed
- **Pulse per-team cache** — cache key now includes `my_team_id`. Previously every teammate in a league got Team A's personal view, including Team A's Claude narrative, because the cache key was league-scoped. Now each team generates and caches its own Pulse payload (`pulse:{league}:{team}:{season}:{gt}:{date}`).
- **"Playoff points" label pre-playoffs** — when Hot/Cold fell back to regular-season leaders, the card still labelled the totals as "playoff pts" and the narrative cited "90 playoff points" for players who had never played a playoff game. Backend now carries a `hotColdIsRegularSeason` flag through to the UI and the prompt.
- **`rand` crate's `gen` method name** — `gen` is a reserved keyword in recent Rust editions. Calls switched to `r#gen::<f32>()` raw-identifier form. Also enabled the `small_rng` feature for `SmallRng`.

### Removed
- **Cup Contenders card on Insights** — redundant with the rebuilt Bracket and Stanley Cup Odds views. Associated `ContenderSignal` DTO and `compute_cup_contenders` handler deleted.
- **Sleeper Watch card on Insights** — overlapped with Hot/Cold. `SleeperAlertSignal` DTO and `compute_sleeper_alerts` handler deleted.
- **Injury Intel card on Insights** — low-signal Daily Faceoff scrape with heuristic name matching. `InjuryEntry` DTO and `split_headlines_and_injuries` helper deleted.
- **Fantasy Race sparklines on Insights** — moved to Pulse (League Live Board already carries this).
- **Old Series Projections grid** — 16 cards of identical "0-0 TIED · 50%" during tied rounds, no new info over the scoreboard. Replaced by `PlayoffBracketTree`.
- **Normal-approximation rivalry math** — `compute_rivalry`'s Welch-style fallback and the Abramowitz & Stegun `erf` / `normal_cdf` helpers are gone. The exact MC pairwise value is always available.

## v1.6.1 — 2026-04-17

### Removed
- **My Goalies Tonight section** on Pulse — this league doesn't draft or score goalies, so the widget was always empty. Removed the section, the `MyGoalieCard` component, and the backend `compute_my_goalies_tonight` / `derive_start_status` helpers + associated DTOs. `PulseResponse.myGoaliesTonight` is gone; remaining top-down order on Pulse: Series Forecast → Today's Pulse → My Players In Action → League Live Board.

## v1.6.0 — 2026-04-17

### Added
- **Series Forecast hero on Pulse** — per-fantasy-team roster × series grid, each cell color-coded by leverage state (eliminated / facing elim / trailing / tied / leading / closing in / advanced). Headline per team: "N players — X facing elim, Y trailing, Z leading." Heuristic win probability and games-remaining rendered inline. Your team is pinned first with a yellow accent.
- **My Goalies Tonight card on Pulse** — per rostered goalie, shows confirmed/probable/backup status from NHL `gamecenter/{id}/landing` `probableGoalies` / `goalieComparison`, opponent, game start time. "TBD" when NHL hasn't posted goalies yet.
- **League Live Board sparkbars** — 5-day daily-points sparkline per team next to today's delta; my team highlighted. Sourced from `daily_rankings` history, brutalist inline SVG (`<Sparkbars>` component — 15 LOC, zero chart-library dependency).
- **Pulse auto-refresh** — 30s polling when games are live, matching the existing `useGamesData` pattern.
- **Hot + Cold Hands split on Insights** — cold = rostered players with ≤1 point across last-5 games AND ≥3 games played floor (prevents missed-game noise). Grouped by fantasy-team owner.
- **Series Projections card on Insights** — every active playoff team with heuristic "% to advance" and games-remaining. Honest labeling: "historical odds based on series state" (down 0-3 ≈ 5%, tied ≈ 50%, up 3-0 ≈ 95%). No external scraping, no broken-scraper risk.
- **Injury Intel card on Insights** — rostered-player injuries split out of the general news scrape into their own widget. Fantasy-team ownership overlaid when the scraped player name matches a rostered player.
- **Ownership tags on game cards** — "Your team has 3 players in this game" yellow badges on `Today's Watch` game cards.
- **Fantasy Race sparkbars + yesterday delta** — 5-day trend chart and "+N yd" arrow per team row.
- **Series-state badges on Cup Contenders** — "3-1 closing in", "2-2 tied", "1-3 facing elim" labels with color-coded backgrounds and `N% · M left` probability/games-remaining.
- **New `/api/pulse` endpoint** — single-call Pulse data with `tokio::join!` parallel signal computation, cached per `pulse:{league}:{season}:{game_type}:{date}` key.
- **`backend/src/utils/series_projection.rs`** — `classify`, `probability_to_advance`, `games_remaining`, `SeriesStateCode` — reusable across Pulse and Insights.
- **Index migration** — `idx_daily_rankings_team_league_date` speeds up per-team sparkline queries.

### Changed
- **Pulse page rewrite** — top-down layout: Series Forecast → Today's Pulse → My Goalies → My Players In Action → League Live Board. Legacy `hooks/use-pulse-data.ts` replaced by `features/pulse/hooks/use-pulse.ts`.
- **Insights signals** — `InsightsSignals` extended with `coldHands`, `injuryReport`, `seriesProjections`; `ContenderSignal` carries series-state / games-remaining / odds; `FantasyRaceSignal` carries sparkline + yesterday delta; `TodaysGameSignal` carries ownership tags.
- **`/nhl/skaters/top` and draft pool helpers** unchanged from v1.5.0 — series projection logic is additive and isolated.

### Dropped (from original v1.6 scope)
- **MoneyPuck integration** — MoneyPuck's data endpoints require a commercial license and their predictions page is JS-rendered. Replaced with an honest in-house heuristic using historical best-of-7 outcome probabilities. No scraper to break.
- **Daily Faceoff starting-goalies scrape** — NHL `probableGoalies` via `gamecenter/{id}/landing` is the canonical source 24h out; the scrape would add ~2 days of infra for a 6-hour earlier signal. Deferred to v1.6.1 if real-world usage shows users need earlier confirmation.

## v1.5.0 — 2026-04-17

### Added
- **Playoff draft pool** — when `NHL_GAME_TYPE=3`, the draft player pool sources from the 16 playoff team rosters via `/playoff-series/carousel/{season}` + `/roster/{team}/current` instead of the `skater-stats-leaders` endpoint, which returns 0 players until playoff games have been played. Falls back to the top 16 teams from standings if the carousel hasn't been published yet. New helper module at `backend/src/utils/player_pool.rs` is shared with `/nhl/skaters/top`.
- **`PlayerPoolUpdated` WebSocket event** — broadcast when an admin repopulates the pool; draft clients invalidate their player-pool query and see the fresh roster without a manual refresh.
- **Config-derived UI labels** — `APP_CONFIG` exposes `SEASON_LABEL` ("2025/2026 Playoffs"), `GAME_TYPE_LABEL`, and `BRAND_LABEL` ("NHL 2026"), all computed from `VITE_NHL_SEASON` / `VITE_NHL_GAME_TYPE`. Flipping two env vars per side now retargets the whole app to any season or game type.
- **Season/game-type flip workflow documented** in `CLAUDE.md`.

### Fixed
- **Games page missed fantasy overlay** — `useGamesData` was calling `api.getGames(date)` without forwarding `activeLeagueId`, so every game rendered "No fantasy team has players for this team" even when players were rostered. Now forwards the league id and keys the React Query cache by it.
- **Hard refresh dropped the user out of their league** — `LeagueProvider` initialized `activeLeagueId` to `null` and never rehydrated from `localStorage.lastViewedLeagueId`. Global routes like `/games/:date` (which don't run `LeagueShell`) lost the active league on refresh. Lazy state initializer now reads the key on first mount.
- **Hardcoded `game_type=2` in `create_draft_session`** removed — both draft-creation and populate-pool paths now honor the configured `game_type()`.

### Changed
- **Cache hygiene** — response-cache keys for `insights`, `games_extended`, and `match_day` now include `game_type()` so payloads don't collide across a regular-season → playoffs flip. Old keys age out via the existing 7-day cleanup.
- **`/nhl/skaters/top`** — when `game_type=3`, serves from the playoff roster pool (same source as the draft) instead of the empty skater-stats-leaders endpoint.
- **All hardcoded `"2025/2026 Playoffs"`, `"NHL 2026"`, and `"20252026"` literals** in the frontend now read from `APP_CONFIG` (HomePage, RankingsPage, DraftPage, AdminPage, LoginPage, LeaguePickerPage, LeagueSettingsPage, NavBar, TeamBetsTable, PlayerRoster, `api/client.ts`).

## v1.4.0 — 2026-04-15

### Added
- **League-scoped settings page** — `/league/:id/settings` replaces the monolithic admin page for managing a single league's members, draft, and player pool
- **Rich league preview for non-members** — visiting a league via invite link now shows members list, draft status, and a prominent join CTA
- **Join from league picker** — non-member public leagues show a "Join" button directly on the card alongside "View League"
- **League-specific invite links** — "Copy Invite Link" now copies `/league/:id` instead of a generic `/join-league` URL
- **Login return-to support** — after signing in via an invite link, users are redirected back to the league page
- **Health check endpoints** — `GET /health/live` and `GET /health/ready` (verifies DB connectivity)
- **Typed config module** — `Config::from_env()` loads all settings eagerly at startup with clear panic messages for missing vars
- **DB authorization helpers** — `verify_league_owner`, `verify_user_in_league`, `get_league_id_for_draft/team/player`

### Changed
- **Create league flow** — now prompts for team name alongside league name, auto-joins the creator, and navigates to the league dashboard
- **Admin page simplified** — shows only "Create League" form and a grid of owned leagues linking to per-league settings
- **NavBar** — "Manage Leagues" renamed to "My Leagues"; new "League Settings" link for league owners
- **`/join-league` retired** — now redirects to `/league/:id` or `/` (old links still work)
- **Backend authorization hardened** — all draft, league member, team, and player endpoints now verify the caller is a league member or owner (previously only checked authentication)
- **JWT secret wrapped in `secrecy::SecretString`** — prevents accidental logging of the secret
- **Password hashing moved to blocking threads** — `hash_password`/`verify_password` run on `spawn_blocking` to avoid stalling the async runtime
- **HTTP middleware stack** — added gzip compression, 30s request timeout, 1MB body limit, configurable CORS origins
- **Graceful shutdown** — server handles SIGTERM/Ctrl+C cleanly
- **Structured logging** — JSON format via `LOG_JSON=true`, env-filter support via `RUST_LOG`
- **Error handling** — new `Conflict` (409) variant; NHL API errors no longer leak internal details

### Fixed
- **Total picks display** — admin draft stats now show correct pick count (was off-by-one showing 0-based index) and includes sleeper picks in the total

## v1.3.1 — 2026-04-10

### Fixed
- **Leagues nav link** — always visible for logged-out users browsing a league, so they can navigate back to the league picker

## v1.3.0 — 2026-04-10

### Added
- **Global Insights page** — Insights now accessible at `/insights` without selecting a league; shows NHL-wide game previews, hot players, and contenders

### Changed
- **Nav rework based on context** — navigation adapts to three states:
  - No league selected: Leagues, Games, Insights, Skaters
  - League selected, no team: Dashboard, Insights, Games, Stats, Skaters (Pulse hidden)
  - League selected, has team: Dashboard, Pulse, Insights, Games, Stats, Skaters
- **Leagues nav link** — now visible for all users when no league is selected (was login-only)

### Fixed
- **Insights game card header** — team name, record, and streak info stacked vertically so long names like "Maple Leafs" no longer push the record out of alignment

## v1.2.1 — 2026-04-09

### Fixed
- **Insights mobile layout** — game card player stats and goalie info no longer float/jump on narrow screens; stats stack vertically on mobile (side-by-side on desktop), player names truncate reliably, goalie record and save stats split into stable lines

## v1.2.0 — 2026-04-09

### Added
- **Pulse page** — new quick-glance dashboard (Dashboard > Pulse in nav) showing: my team rank/points/today, players grouped by tonight's games with start times, and league board with opponent activity
- **Sleeper delete endpoint** — `DELETE /api/fantasy/sleepers/:id` for removing sleeper picks
- **Sleeper management in admin** — sleepers now visible in Player Management with yellow badge and Remove button
- **Makefile improvements** — `make run` waits for backend to be ready before starting frontend; `make cache-clear` to wipe response cache

### Changed
- **Nav restructure** — Dashboard, Pulse, Insights, Games, Stats, Skaters in main nav; Teams moved to dropdown alongside Browse Leagues and Manage Leagues
- **Games page simplified** — removed My League and Player Matchups tabs; Games page now shows only NHL game cards
- **Insights narratives** — Claude no longer prefixes game narratives with matchup labels (e.g. "CBJ @ BUF:"); streak labels now readable ("Won 2" instead of "W2")
- **Insights layout** — game cards in 2-column grid on desktop
- **Fantasy summary and team cards** — redesigned with consistent black/white headers, compact player rows
- **Player matchups** — team logos instead of colored squares, compact VS rows
- **Pulse headers** — white background with black text, consistent across all sections

### Fixed
- **Draft finalize propagation** — non-owners now see sleeper round transition without page reload (invalidateQueries on sessionUpdated WS event)
- **Player delete** — admin page now correctly deletes players by NHL ID (was sending NHL ID to an endpoint expecting DB ID)
- **Admin player count** — includes sleeper in the total count per team
- **Admin player list** — correctly parses nested NHL-team-grouped API response instead of expecting flat array
- **AdminPage infinite loop** — fixed useEffect dependency on `members` array reference causing re-render loop
- **Dashboard post-draft-delete** — shows rankings instead of "Draft Hasn't Started" when teams have data but draft session was deleted
- **Sleeper visibility** — sleeper stays visible in admin even when all regular players are removed

### Removed
- GameTabs, FantasySummary, FantasyTeamCard, PlayerComparison, PlayerWithStats, FantasyTeamSummary components
- useFantasyTeams hook
- matchDay duplicate components

## v1.1.0 — 2026-04-08

### Fixed
- Draft state not propagating to other participants — finalize (sleeper transition) and complete (draft done) now update all clients in real-time without requiring a page reload. Root cause: LeagueContext and useDraftSession cached the same draft session under different React Query keys, so WebSocket updates only reached one of them.
- Makefile `run` target now always uses local dev database (`.env.development`), never connects to production
- Supabase local config slimmed to Postgres-only (no auth, storage, realtime, studio, edge runtime) — faster startup, fewer Docker images

### Changed
- Backend loads `.env` via standard dotenv (`.env.development` is copied to `.env` by Makefile)

## v1.0.0 — 2026-04-08

Initial stable release as a monorepo (`backend/` + `frontend/`).

### Features
- **NHL API integration** with in-memory caching (12 endpoint-specific TTLs) and semaphore-based rate limiting
- **Fantasy leagues** — create/join leagues, manage teams, snake draft with real-time WebSocket
- **AI-powered insights** — per-game narratives via Claude API, with standings, NHL Edge analytics, yesterday's scores
- **Playoff tracking** — daily rankings, historical performance, playoff bracket
- **Scheduled jobs** — rankings at 9am/3pm UTC, insights pre-warming at 10am UTC, weekly cache cleanup
- **JWT authentication** with Argon2 password hashing

### Bug Fixes (post-v1.0.0, pre-release)
- Admin endpoints now require JWT + `is_admin` check
- Player matching uses `nhl_id` (primary) with last-name fallback instead of fragile substring matching
- DST timezone handling uses `chrono-tz` America/New_York instead of crude month-range approximation
- Startup backfill runs in background (non-blocking) so Fly.io health checks pass
- Single WebSocket connection per draft page (was 3 independent connections)
- `daily_rankings` UNIQUE constraint includes `league_id`; goals/assists columns now populated
- Weekly cleanup of `response_cache` entries older than 7 days
- Orphaned sleeper scoping fixed (no longer leaks across leagues)
- Server-side WebSocket ping every 30s for keepalive through proxies
- `window.location.reload()` replaced with React Query invalidation / React Router navigation
- LeagueContext refactored from raw useEffect to React Query (caching, dedup, shared query keys)
- Season config moved to env vars (`NHL_SEASON`, `NHL_GAME_TYPE`, `NHL_PLAYOFF_START`, `NHL_SEASON_END`)
- Removed unused `@supabase/supabase-js` dependency and dead `DEFAULT_QUERY_OPTIONS` config
- Headline scraper logs warning when returning 0 results
- `search_players` searches all teams instead of stopping after first match

### Infrastructure
- Monorepo structure: `backend/` (Rust/Axum) + `frontend/` (React/Vite)
- Local dev via Supabase CLI (`make run` starts Postgres + backend + frontend)
- `.env.development` for local, Fly.io secrets for production
- Makefile with `run`, `dev`, `db-start`, `db-reset`, `install`, `check`, `deploy`
- Technical documentation in `docs/` (architecture, API reference, data flow, caching, operations)
