# Changelog

All notable changes to Fantasy Puck are documented here.

## Unreleased

## v1.23.2 / v1.19.2 — 2026-04-23 (BE v1.23.2 / FE v1.19.2)

### Fixed — Skaters leaderboard stat and ownership columns

The playoff skaters leaderboard now returns the full table line the
frontend renders: goals, assists, points, PIM, plus/minus, and
average TOI. The Skaters page also passes the active league id to the
leaderboard endpoint, so rostered players show their assigned fantasy
team and link to that team page instead of rendering blank ownership.

## v1.19.1 — 2026-04-21 (frontend)

### Changed — Roster Breakdown Skater column compacted

Removed the player headshot, the separate Team column, and the
position. The Skater cell now renders the abbreviated name plus team
on one line (`A. Tuch · BUF`), freeing roughly 10 rem of horizontal
space for the 13 stats columns so the table breathes without needing
horizontal scroll.

## v1.23.1 — 2026-04-21 (backend)

### Fixed — Claude timeout bumped to 90 s for team-diagnosis prompt

The team-diagnosis prompt sends the full per-player breakdown plus a
recent-games strip for every rostered skater, and Sonnet 4.6 can
comfortably take 45–60 s to produce the three 2200-token sections.
The previous 30 s cap caught sporadic timeouts on the daily prewarm;
the fallback path is graceful (empty narrative, static UI fallback)
but the cached row ends up empty instead of populated. 90 s gives
the prompt headroom while remaining well under any edge/proxy cap.

## v1.23.0 / v1.19.0 — 2026-04-21 (BE v1.23.0 / FE v1.19.0)

### Changed — Pulse rebuilt around Your Read, Roster Breakdown, and Your League

Pulse's narrative-only "Your Read" is replaced by three descriptive
blocks that render from one API round-trip:

- **Your Read** — rank/gap strip, concentration-by-NHL-team chips,
  and a three-section markdown narrative (Where You Stand /
  Player-by-Player / What to Expect) generated per
  `(league, team, hockey-date)`. The voice is deliberately
  descriptive, not prescriptive — the roster is locked for the
  playoffs, so no "start/sit/drop" phrasing, no action items, just
  what happened, why, and what the model expects.
- **Roster Breakdown** — per-player table with the full playoff box
  line (GP, G, A, P, SOG, PIM, +/-, HIT, TOI/game), a projected PPG
  from the existing Bayesian blend, a letter grade (A–F) that scores
  actual vs expected via a Negative-Binomial variance model matching
  the Monte Carlo, an expected-remaining-points figure sourced from
  the cached race-odds payload, and a descriptive status bucket
  (Ahead, On Expected, Due, Below Expected, Fading, Not In Lineup,
  Team Out, Too Early). Sortable on every column.
- **Your League** — leader, points distribution across all teams,
  and the top-3 projected finishers from the Monte Carlo with each
  team's largest NHL stack and that team's cup-win probability.

The old "League Live Board" with sparklines is retired from Pulse.
The same data still drives the Dashboard's Live Rankings widget.

### Added — Per-player performance grading module

New pure-domain module that converts a skater's projected PPG, actual
playoff points, games played, projection availability multiplier, and
series state into a letter grade, a z-score, an expected-remaining
impact figure, and one of eight descriptive buckets. Gated below two
games played to avoid judging a player on a single goose-egg. Live
poller invalidates the cached narrative on every game-end transition
involving a rostered player, so the next Pulse load regenerates
against the final score.

### Added — Regular-season coverage via per-team club-stats fan-out

The NHL skater-stats-leaders endpoint only captures the top-25 per
category, which silently zeroed out projections for every depth
player on a fantasy roster. The meta-poller's 24-hour roster walk
now also hits `/v1/club-stats/{team}/{season}/2` for every NHL club
and upserts every skater who dressed into `nhl_skater_season_stats`.
Complete regular-season PPG coverage for every rostered player.

A matching admin endpoint, `GET /api/admin/refresh-club-stats`, fans
out the same walk on demand. Exposed in the admin dashboard as
"Refresh Club Stats".

### Added — Per-team Pulse narrative pre-warm

The daily 10:00 UTC prewarm cron plus the on-demand
`GET /api/admin/prewarm` now pre-generate the Pulse diagnosis
narrative per `(league × team)`, so the first Pulse load of the day
is a warm cache hit rather than a Claude round-trip.

### Fixed — TOI wasn't being persisted from the boxscore

The player-game-stats upsert was hard-coding NULL for `toi_seconds`,
even though the NHL boxscore returns per-skater time on ice as
`"MM:SS"`. The boxscore model now parses the field, and the upsert
stores integer seconds with a COALESCE so retries don't clobber a
previously filled value. `GET /api/admin/rehydrate` backfills
historical rows on demand.

### Fixed — Grade gating no longer reports healthy-looking falsehoods

Below two playoff games played, the classifier now reports a neutral
"Too Early" bucket instead of collapsing into the green "On Expected"
label — a zero-game roster no longer reads as a passing grade.

### Fantasy Team Detail page

Unchanged. Existing table shape and columns preserved; all new
per-player breakdown and diagnosis work lives on Pulse.

## v1.18.3 — 2026-04-20 (frontend)

### Changed — Brand header shortened to "FANTASY NHL 2026"

The NavBar and login page brand label previously included the current
game-type affix, rendering as "FANTASY NHL PLAYOFFS 2026" during the
playoff window. Dropped the game-type word from `BRAND_LABEL` so the
header now reads "FANTASY NHL 2026" year-round.

## v1.18.2 — 2026-04-20 (frontend)

### Fixed — Bracket "Strength" tooltip described the wrong formula

The (i) tooltip next to each team's Strength value on /insights claimed
the number was a 70/30 blend of regular-season standings points and
last-10-games pace. During the playoffs that's not what the bracket
reads — `enrich_projections` swaps to the dynamic playoff Elo (seeded
from RS points, then updated after every completed playoff game),
which is why the displayed values are centered on 1500 rather than in
the NHL-points range. Rewrote the copy to describe the Elo rating
accurately.

### Changed — Strength tooltip is now a clickable popover

Replaced the native `title` attribute with the same brutalist popover
pattern used by Daily Wins / Daily Top 3 on /stats: click the (i),
popover opens with a close button, click again or hit the X to
dismiss. Only one popover open at a time across the whole bracket.

## v1.18.1 — 2026-04-20 (frontend)

### Changed — /stats is one standard table primitive end to end

The NHL Teams and Top 10 Rostered Skaters sections shipped as
bespoke tables with their own chrome, which broke the page's
visual rhythm the moment you scrolled past Season Overview.
Rebuilt both on `RankingTable` with column definitions in
`components/rankingsPageTableColumns/`, same contract as the
other three tables. Blue "League Stats" header block retired.

Final /stats order is Season Overview → Daily Fantasy Scores
→ Playoff Stats → NHL Teams We Roster → Top 10 Rostered Skaters.
NHL Teams sorts by Playoff Pts DESC by default; columns are
NHL team, Playoff Pts, Rostered, Top Skater. Top 10 Rostered
Skaters sorts the same way, columns headshot + name, NHL team,
Playoff Pts, Rostered By.

### Fixed — Daily Fantasy Scores date picker was invisible

The prev-day / date-trigger / next-day / Yesterday buttons all
rendered `text-white` on `bg-white/10`, so the whole control strip
was effectively invisible against the white table header. Restyled
to the brutalist black-on-white pattern used by the Games page's
`DateHeader`: `border-2 border-[#1A1A1A]` with a black-on-hover
invert, Yellow `#FACC15` fill on the Yesterday button.

### Added — Daily Fantasy Scores picker respects the playoff window

`RankingTable` / `RankingTableHeader` now accept `minDate` /
`maxDate` (YYYY-MM-DD). Prev/Next disable at the bounds, the
calendar constrains its selectable days, and the Yesterday button
clamps so day 1 of the playoffs doesn't kick the picker to a
pre-playoff date. `RankingsPage` passes `APP_CONFIG.PLAYOFF_START`
and `APP_CONFIG.SEASON_END`, matching how `GamesPage` already
clamped its own picker.

### Changed — Yellow "2025/2026 Playoffs" badge retired

That context belongs in the nav bar, not on every table. Brand
label updated from `NHL 2026` to `NHL Playoffs 2026` when the app
is in game_type=3 so the header reads "FANTASY NHL PLAYOFFS 2026".
`dateBadge={APP_CONFIG.SEASON_LABEL}` removed from Season Overview,
Playoff Stats, Fantasy Team Roster, and Skaters from NHL Teams.

### Changed — CLAUDE.md and AGENTS.md codify the standardisation rule

New section: before hand-rolling a table, card, or header, use the
existing primitive. Visual consistency across the app is load-bearing;
bespoke components next to standard ones read as broken.

## v1.22.0 / FE v1.18.0 — 2026-04-20 (backend + frontend)

### Added — League Stats section on /stats

Two league-wide tables at the bottom of the Stats page.

**NHL teams we roster.** One row per NHL team with at least one
rostered fantasy player, sorted by count of rostered players.
Columns: NHL team (logo + name + abbrev), rostered count, that NHL
team's total playoff fantasy points so far, and that team's top
playoff scorer (photo + name + pts). Useful for spotting roster
concentration ("we're heavy on BUF and COL") and seeing how those
NHL teams are actually scoring.

**Top 10 rostered skaters.** The league's own leaderboard —
skaters rostered by any fantasy team, sorted by playoff fantasy
points DESC. Columns: headshot, name, NHL team, the fantasy team
that rosters them. Your own team is highlighted.

Backend: new `GET /api/fantasy/league-stats?league_id=...` handler
in `league_stats.rs`, four concurrent reads against
`nhl_player_game_stats` joined to `nhl_games` (completed playoff
games only). SQL lives in `infra/db/league_stats.rs`.

## v1.17.7 — 2026-04-20 (frontend)

### Changed — Race Odds drops the normalized-bar panel; table is the sole view

Two visuals were competing to tell one story. The bar panel encoded
each team's win probability scaled against the league leader while
the table beneath it carried the same win %, plus current points,
projected mean, p10–p90 range, top-3 %, and head-to-head "You beat".
Because the bar was normalized to the leader, a team with 21% win
probability looked ~78% full — bigger-bar-is-better intuition broke
against the numbers right next to it.

Deletes `LeagueRaceBoard` and renders only `LeagueRaceTable`. Same
data, one encoding, nothing to reconcile between two tracks.

## v1.17.6 — 2026-04-20 (frontend)

### Fixed — Dashboard "Yesterday's Rankings" truncated at 7

The dashboard card capped the table at 7 rows, so the bottom half of
the league (ranks 8-14) silently vanished from the home page even
though the Stats page's equivalent "Daily Fantasy Scores" table
rendered the full 14. Both read the same `api.getDailyFantasySummary`
payload, so the split was purely a dashboard prop. Drops the `limit`
so every rostered team is visible.

## v1.21.11 — 2026-04-19 (backend)

### Fixed — Live Rankings "TODAY" column zeroed after midnight UTC

The pulse handler reads `points_today` from the last entry of each
team's live sparkline. That sparkline is built by
`get_team_sparklines_with_live`, which was computing its "today"
edge via `Utc::now().date_naive()` — so at 00:12 UTC on Apr 20 it
asked the live view for Apr 20 rows and got nothing, zero-padded
the rightmost slot, and reported `points_today = 0` for every team
even though the Apr 19 ET slate had finals and live games on the
board. The companion frontend fix in v1.17.5 got the *right day's*
schedule showing; this gets its scoring totals back.

`v_daily_fantasy_totals.date` is keyed on `nhl_games.game_date`,
which is already ET-anchored by the schedule ingest, so the
correction is a one-line switch to
`Utc::now().with_timezone(&America::New_York).date_naive()` — the
pattern `hockey_today()` and the meta-poller already use.

Two adjacent consistency fixes while in the area:
`insights::build_todays_signals` used UTC in its date-parse fallback
path; `main.rs` startup uses UTC to gate the initial backfill
against the playoff window. Neither caused a user-visible bug at
the current schedule (both sit well inside the UTC/ET-agree window
or never fire) but both now match the ET rule.

## v1.17.5 — 2026-04-19 (frontend)

### Fixed — Dashboard and Games page now anchor "today" to Eastern Time

The dashboard's Live Rankings banner and the Games page both rolled
forward to the next day's slate the moment UTC (or the viewer's local
clock) crossed midnight, even while that night's NHL games were still
in flight. The backend already keys its schedule on the ET calendar
date via `hockey_today()`, matching how NHL.com and the league itself
define a "game day"; the frontend was deriving "today" from UTC
(`Live Rankings` link) or from the browser's local timezone (Games
page default, "Today's Games" nav link, "Today"/"Yesterday" buttons),
so the two drifted apart as soon as ET and UTC/browser-local fell on
different calendar days.

`getHockeyDateToday()` and `getHockeyDateYesterday()` now format
through `Intl.DateTimeFormat` pinned to `America/New_York`. Every
"today" / "yesterday" derivation across the dashboard, nav bar,
Games page, Daily Rankings, Admin panels, and the rankings hooks
routes through them. A viewer in Halifax at 21:12 ADT or in London
at 03:00 BST now sees the same Apr 19 slate — and the same Live
Rankings leaderboard — as a viewer in Toronto.

## v1.17.4 — 2026-04-19 (frontend)

### Fixed — Live Rankings rendered upside-down

Switching the body to `RankingTable` in v1.17.3 picked up its
default sort (`"desc"` on the first column, which resolved to
`rank`), flipping the list so rank 14 sat on top and rank 1 at
the bottom. Live Rankings pre-sorts its rows by today's points
descending and stamps `rank` accordingly, so the right fix is to
pin `RankingTable` to the precomputed order via
`initialSortKey="rank"` + `initialSortDirection="asc"`.

## v1.17.3 — 2026-04-19 (frontend)

### Changed — Live Rankings body matches Overall Rankings

Previous pass hand-rolled the table grid so row heights and the
rank column didn't match Overall Rankings sitting right below it
— team names truncated to "Boston ..." at the default column
width, and the colored rank badge (yellow/silver/bronze) was
missing. Now renders the body through the shared `RankingTable`
component with its own column config
(`useLiveRankingsColumns` — Rank · Team · Today · Players · Games).
Rank badges, wrap-capable team-name cells, and row heights match
Overall / Yesterday / Sleepers.

The red banner + pulsing white dot + "→ Live Games" button stay —
they're now slotted into the same outer border as the table body
via a new `customHeader` prop on `RankingTable`. Default header
behavior is unchanged for every other caller.

## v1.21.3 — 2026-04-19 (backend) / v1.17.2 (frontend)

### Changed — Live Rankings gets its own layout

First try used the shared `RankingTable` to match Overall Rankings
visually; that cost the red header and pulse dot the dashboard needs
to telegraph "this only exists while games are live." This pass
drops `RankingTable` and lays out the table directly:

- Red header (`#EF4444`) + white pulsing dot, matching the LIVE pill
  on the Games page. "→ Live Games" button stays on the right.
- Sort reversed: highest-points-today at the top (was upside-down,
  rank 14 at the top).
- Columns: Rank · Team · **Today** · Players · **Games**. "Total"
  dropped (already on Overall Rankings below), "Games" added.
- Games cell lists the NHL matchups this fantasy team has a stake
  in (either side rostered). The rostered side(s) render bold.
  E.g. "**CAR**–OTT, MIN–**DAL**" when the team rosters a Hurricane
  and a Star. Teams with no stake in any game tonight show "—".

Backend: `PulseResponse` gains `gamesToday: Vec<GameMatchup>` — the
flat list of today's home/away abbrev pairs. Populated from the
`games_today` vec already computed in `generate_pulse`. FE derives
per-team rosters from `seriesForecast.cells` and intersects.

Also dropped the short-lived `useLiveRankingsColumns` helper — the
section no longer uses `RankingTable`, and the column shapes are
inlined in `LiveRankingsTable`.

## v1.17.1 — 2026-04-19 (frontend)

### Changed — Live Rankings renders through `RankingTable`

First pass used a custom compact-row layout so row heights and
column widths didn't match the Overall Rankings table below. Now
renders through the shared `RankingTable` component with its own
column set (`useLiveRankingsColumns` — Rank / Team / Today / Active
/ Total). Section matches Overall / Yesterday / Sleepers visually.

- New `useLiveRankingsColumns` for the table shape.
- `RankingTable` gains an optional `alwaysShowViewAll` prop so the
  top-right "Live Games" link renders even when no `limit` is set
  and the list isn't truncated. The default behavior (show only on
  overflow) is unchanged for existing callers.

## v1.17.0 — 2026-04-19 (frontend)

### Added — Live Rankings on the dashboard

New section at the top of `/league/:id` that appears only while any
game on the slate is `LIVE|CRIT`. Lists every league team with at
least one rostered skater in tonight's NHL games, sorted by today's
live points (from `v_daily_fantasy_totals`). Teams with zero active
skaters are omitted.

- Red header with a pulse dot mirrors the "LIVE" treatment on the
  Games page.
- Right-side "→ Live Games" button links to `/games/{today}`.
- The caller's own team is highlighted yellow (same rule as the
  Pulse League Live Board).
- Hidden on off-days and pre-puck-drop; Overall Rankings sits back
  at the top.

Backed by the existing `/api/pulse` payload (same hook as Pulse),
so no new endpoint and no re-derivation. Auto-refresh follows the
Pulse cadence — updates ~every 60 s while games are live.

## v1.21.2 — 2026-04-19 (backend)

### Fixed — `LIVE · P12` period render

`get_game_data` formatted the period into a composite string like
`"2 Period"` and handed that to the live poller, which then tried to
`parse::<i16>()` it for the `period_number` column. That parse always
failed, so `period_number` stayed at whatever stale value the meta
poller had seeded (usually `1`) while `period_type` was overwritten
with the composite string. `format_period(1, "2 Period")` emitted
`"1 2 Period"`; the frontend's digit-stripper reduced that to `12`,
shown as `LIVE · P12`.

Fixed at the source: `GameData` now carries `period_number` and
`period_type` as separate fields. The live poller writes them
directly into the matching mirror columns, so a tick in period 2
stores `(2, "REG")` and `format_period` produces the expected
`"2 Period"` → `P2`.

Stuck rows self-correct on the next live-poller tick after deploy
(≤60 s). No manual rehydrate required.

## v1.21.1 — 2026-04-19 (backend) / v1.16.1 (frontend)

### Fixed — fun-ui components rendered unstyled

Arbitrary-value Tailwind classes baked into the `@gruberb/fun-ui`
bundle (`border-[var(--color-brutal-black)]`,
`border-t-[var(--color-brutal-yellow)]`, etc.) were missing from the
output CSS. Tailwind v4 skips `node_modules/` when scanning for class
names, and the `@source` directive didn't pick up the deep path
either.

Symptom: `LoadingSpinner` was invisible because its border ring and
yellow accent class weren't emitted. Other fun-ui components inherited
the same issue (modals, badges).

Fix: new `frontend/src/funUiSafelist.ts` lists every arbitrary-value
class fun-ui references. Tailwind scans `src/` by default, so the
classes are emitted without any additional config. Regenerate the
list by grep'ing the fun-ui bundle — the file's header comment has
the one-liner.

### Fixed — live poller: stuck LIVE rows from past dates

The live poller only queried `nhl_games` rows with `game_date = today`
and state in (`LIVE`, `CRIT`, `PRE`). A row stuck in `LIVE` from a
previous date (process restart, rate-limit blip, whatever) would
never get re-checked, so the Games page kept rendering a finalised
game as live until someone manually ran `/api/admin/rehydrate`.

New `nhl_mirror::list_games_needing_poll` sweeps `LIVE|CRIT` rows
regardless of date and adds today's `PRE` rows on top. Old function
(`list_live_game_ids_for_date`) kept untouched — it's still used by
the daily-rankings safety gate, which genuinely wants per-date
semantics.

## v1.16.0 — 2026-04-19 (frontend)

### Changed — Consolidate onto @gruberb/fun-ui

Three custom components replaced with their name-matching fun-ui
equivalents. Props are identical, so every call-site works unchanged;
deleted files live on in fun-ui.

- `@/components/common/LoadingSpinner` → `{ LoadingSpinner } from "@gruberb/fun-ui"` (14 files)
- `@/components/common/ErrorMessage`  → `{ ErrorMessage } from "@gruberb/fun-ui"` (9 files)
- `@/components/common/PageHeader`    → `{ PageHeader } from "@gruberb/fun-ui"` (8 files)

Also: the admin `ConfirmDialog` now wraps fun-ui's `Modal` internally
(same public prop shape — admin panels unchanged). And
`@gruberb/fun-ui/styles` is imported from `index.css` so fun-ui's
`brutal-*` classes and CSS custom properties resolve at build time.

The three local component files (`LoadingSpinner.tsx`,
`ErrorMessage.tsx`, `PageHeader.tsx`) are deleted. Next time fun-ui
bumps, the whole app picks up the change.

## v1.15.1 — 2026-04-19 (frontend)

### Changed — Games header

- Added `@gruberb/fun-ui` (^0.1.0) as a frontend dependency. First use:
  the brutalist `LiveIndicator` replaces the bespoke "Live —
  auto-updating" pill on `/games`. Matches the style of the rest of
  the app and gives us a single source of truth as more surfaces adopt
  fun-ui components.
- Removed the manual **Refresh** button. `useGamesData` already polls
  via React Query's `refetchInterval` — 30 s cadence while any game is
  `LIVE|CRIT`, auto-stops when the slate finalises. Manual refresh was
  redundant and sat there even on off-days, where it did nothing
  useful.

## v1.15.0 — 2026-04-19 (frontend)

### Changed — Split `/admin` into `/my-leagues` and `/admin`

The old `/admin` page served both purposes: every user saw a
"create league + your leagues" layout, and admins got a small
"Super Admin" badge that exposed an "All Leagues" grid. Clean split:

- **`/my-leagues`** — new route for every signed-in user. Create a
  league + manage the leagues you own. No admin badge, no
  cross-league view.
- **`/admin`** — new admin-only dashboard. Non-admins who navigate
  directly are redirected to `/my-leagues`. Exposes every
  `/api/admin/*` endpoint as a one-click button with inline JSON
  response rendering:
  - Invalidate Cache (scope: `all` / `today` / date picker)
  - Reprocess Daily Rankings (date)
  - Prewarm Caches
  - Rehydrate NHL Mirror (confirm first — long-running)
  - Backfill Historical Playoffs (start + end)
  - Rebackfill Carousel (season)
  - Calibrate (season; Brier / log-loss per round rendered as a
    summary table above the raw JSON)
  - Calibrate Sweep (season + advanced hyperparameter grids;
    confirm first — grid capped at 200 cells)
  - Cross-league "All Leagues" list at the bottom.

### Added

- `NavBar` dropdown shows a new **Admin** entry (yellow-on-black
  styling) only when `profile?.isAdmin === true`.
- Shared admin UI infrastructure under `frontend/src/features/admin/`:
  `AdminActionCard` + `ResultPanel` (pretty JSON, copy button, timestamp,
  ok/err accent) + `ConfirmDialog` for destructive actions.

### Backend

No changes. Every admin endpoint already existed in
`backend/src/api/handlers/admin.rs` and already enforced `is_admin`;
this release just surfaces them in the UI.

## v1.21.0 — 2026-04-19 (backend) / v1.14.0 (frontend)

### Changed — Pulse "Your Read" replaces "Where You Stand"

The narrative now reads like a friend who watches every game telling
you honestly whether your roster is live. Three sections with H3
sub-headings:

- **The Read** — verdict + the one thing the roster is built on and the
  one thing working against it.
- **Swing Pieces** — 3–5 players whose performance decides the outcome,
  each with a single clause on why.
- **Rival Risk** — leader's stack profile contrasted with the caller's,
  ending on "your path is better / worse / even".

Prompt changes:

- `PulseResponse` gains `nhl_team_cup_odds` — a best-effort lift from
  the cached `/api/race-odds` payload keyed by NHL abbrev. The
  narrator uses these to cross-reference stack concentration vs cup
  win probability. Empty when the morning Monte Carlo hasn't warmed
  the cache yet (narrator skips odds phrasing in that case).
- Headline block fed to Claude adds: caller's stack profile (NHL
  teams → player count → cup %), path diversity count, leader's stack
  profile, and an alive/trailing/eliminated summary of the caller's
  skaters.
- Two few-shot examples pin the voice (post-day-1 and zero-state).
- `max_tokens` raised 1500 → 2200 to fit the three-section output
  without clipping the Swing Pieces list on deeper rosters.

Frontend: `PulseNarrative` now parses `### Heading` and `- bullet`
markdown on top of the existing `**bold**` rule. Section header
renamed `Where You Stand` → `Your Read`.

### Added — Insights "Last Night" recap

A new block at the top of `/insights` that summarises the previous
hockey-day's completed games in a Daily Faceoff voice.

- New `LastNightGame` signal (per-game: home/away, final score,
  post-game series state, top 3 scorers) computed from yesterday's
  `nhl_games` + `nhl_player_game_stats`, clamped by `playoff_start()`.
- New `last_night` narrative — one `### Sub-heading` per covered
  game, followed by a 2–4 sentence paragraph.
- Frontend renders per-game cards (headline, score, series state,
  top scorers) plus the narrative with H3 parsing.

### Fixed

- **UTA team link** pointed at `nhl.com/utahhc` (the provisional slug
  from the franchise's first season). They rebranded to **Utah
  Mammoth** and live at `/utah/`. `NHL_TEAMS_BY_ABBREV[UTA]` now
  reflects that — full name, short name, and slug.
- **Daily Fantasy Scores / Yesterday's Rankings** column order was
  `Goals · Assists · Points`. Reordered to `Points · Goals · Assists`
  so the sort key lands in the first numeric column.

### Notes

The Pulse and Insights narratives are cached per
`{league}:{team}:{date}` and `{league}:{date}` respectively. After
deploy, existing cached entries keep serving until midnight ET unless
busted. Use `GET /api/admin/cache/invalidate?scope=today` to force
a fresh generation.

## v1.20.6 — 2026-04-19 (backend) / v1.13.4 (frontend)

### Fixed — Pre-playoff data leaking into playoff surfaces

`daily_rankings` is append-only across game types and seasons, so
`get_daily_ranking_stats` was counting regular-season daily wins and
top-3 finishes in the Season Overview's playoff table. The same concern
applied to `list_league_team_season_totals`: its `g.season = $2 AND
g.game_type = $3` predicates lived on a LEFT JOIN's ON clause, so a
non-matching row nulled `g` but still let `pgs.points` into the SUM.

- New `DateWindow` value type in `infra::db`, wrapping optional
  `min_date` / `max_date` bounds, threaded into the affected queries.
- New `api::current_date_window()` — returns
  `between(playoff_start, season_end)` in playoff mode, unbounded
  otherwise. Every handler that aggregates across date-keyed history
  passes it through (`team_stats`, `rankings`, `pulse`, `race_odds`).
- `list_league_team_season_totals` moved the season / game_type
  predicate and the new date window into CASE-gated SUMs so the LEFT
  JOIN still returns every fantasy team (including ones with zero
  rostered appearances) without letting off-mode stats into totals.

### Changed — Playoff window as first-class config

- Added `VITE_NHL_PLAYOFF_START` / `VITE_NHL_SEASON_END` mirroring the
  existing backend env vars. Exposed as `APP_CONFIG.PLAYOFF_START` /
  `APP_CONFIG.SEASON_END` in `frontend/src/config.ts`.
- `DateHeader` accepts optional `minDate` / `maxDate` props; Prev/Next
  buttons disable at the boundary, the inline DatePicker greys out
  dates outside the window, and the "Today" button clamps to the
  nearest bound when today is outside the window.
- `GamesPage` and `DailyRankingsSection` wire the playoff window into
  their date controls. `useGamesData` also clamps the initial
  `selectedDate` so stale `/games/:date` URLs from before the cutover
  redirect into the current window instead of fetching an empty slate.

### Changed — Pulse

Renamed the "Latest" column on the Tonight hero card and the League
Live Board to "Today". The value has always been today's running total
from `v_daily_fantasy_totals`; the old label suggested "last completed
day" and read as 0 for most of each hockey day.

### Changed — Stanley Cup Odds table

Logos bumped from `w-5 h-5` to `w-10 h-10` (sm+: `w-12 h-12`), row
padding from `py-2` to `py-3 sm:py-4`, and the team cell text from
`text-xs` to `text-sm`. On small iOS widths the team column was
cramped enough that the 20 px logo was the only way to identify a row;
the larger logo scans faster and the added row height gives it room.

## v1.20.5 - 2026-04-19 (backend)

### Changed - Documentation rewrite

The `/docs` folder has been rewritten from scratch. Eleven files covering backend architecture, database schema, HTTP API, NHL integration, prediction engine, business logic, background jobs, draft system, frontend architecture, and frontend data flow. Indexed by `docs/README.md` with three reader paths (new backend contributor, new frontend contributor, operator). Every claim cites a source file and line number.

Old docs removed: `PREDICTION_MODEL.md`, `PREDICTION_SERVICE.md`, `TECHNICAL-CACHING.md` at the repo root, and nine overlapping files under `docs/` (api-reference, architecture, caching, CALIBRATION, data-flow, DATA-PIPELINE-REDESIGN, operations, PREDICTION_MODEL, PREDICTION_SERVICE, TECHNICAL-CACHING).

### Changed - CLAUDE.md and AGENTS.md

Added three sections to both files:

- **Documentation** - points at `/docs` and requires same-commit updates when documented behaviour changes.
- **Comments** (bullet under Code style) - comments are written in the voice of a Staff Software Engineer addressing teammates and future maintainers, not in the voice of an LLM. Explain *why*, not *what*; no task references, no hedging.
- **Release workflow** - describes the commit / version / CHANGELOG / push / tag sequence, with explicit instruction to ask the user before running it after a substantial change rather than running it unprompted.

## v1.20.4 — 2026-04-19 (backend)

### Fixed

- `/api/insights` returned a 500 for every request after the v1.20.3 deploy. `RosteredPlayerTag` gained a required `fantasy_team_id` field, but cached payloads written before the deploy lacked it and `serde_json::from_str` failed on every read. `CacheService::get_cached_response` now treats deserialize failures as a cache miss (warn log, return `None`) so the caller regenerates and overwrites the stale row. Self-heals any future DTO schema drift the same way.

## v1.20.3 — 2026-04-19 (backend) / v1.13.3 (frontend)

### Changed — Insights "What to Watch" deep-linking

Every entity mentioned on the Insights game card is now a link:

- **NHL team headers** (away + home) link to `nhl.com/{slug}` — existing `getNHLTeamUrlSlug` helper.
- **Players to Watch leaders** (points / goals / assists head-to-head) link to `nhl.com/player/{id}`. `PlayerLeader` DTO adds an optional `player_id` field, populated from the NHL landing response's `playerId`. Missing IDs render as plain text — no breakage when the upstream omits the field.
- **Fantasy teams** in the `RosteredStakesTable` link to `/league/:leagueId/teams/:teamId` inside the active league. `RosteredPlayerTag` DTO adds `fantasy_team_id` so the frontend doesn't need a name-based lookup. Outside a league context (global `/insights` route) rows render as plain text.

Narrative text is still plain — auto-linking Claude's prose against the signal payload is punted to a follow-up; the risk of mismatching partial names or missing entities is non-trivial and not worth the complexity for this pass.

## v1.20.2 — 2026-04-19 (backend)

### Fixed

- Season Overview table's "Daily Wins" and "Daily Top 3" columns rendered as 0 for every team regardless of actual results. `get_daily_ranking_stats` referenced a non-existent `daily_points` column (the `daily_rankings` table's column is `points`); the SQL errored on every call and the handler's `unwrap_or_else(|_| Vec::new())` swallowed the failure silently. Fixed the column reference and promoted the fallback to a `warn!` log so a future SQL regression surfaces in the server logs instead of hiding as a column of zeros.

## v1.13.2 — 2026-04-19 (frontend)

### Changed

- Insights "What to Watch Today" game card: the yellow fantasy-team chip cluster under each matchup is replaced with a compact two-column mini-table (`RosteredStakesTable`). Shows team name, a scaled horizontal bar representing roster count relative to the heaviest-invested team in that game, and the raw count. Header tallies total teams and total rostered players across the matchup. In leagues where ten-plus teams own players in a single game, the chip layout wrapped into a wall of uppercase yellow; the table scans linearly and lets you compare counts at a glance.

## v1.13.1 — 2026-04-19 (frontend)

### Fixed

- Daily Fantasy Scores table rendered the empty state ("No daily rankings available for this date") even when the backend returned the full list. Server response is `{ date, rankings: [...] }` but `api.getDailyFantasySummary` was typed as `Promise<RankingItem[]>` and returned the wrapper object unchanged — HomePage and RankingsPage both guard with `Array.isArray(data) ? data : []`, which fell through to the empty array for every non-list response. `getDailyFantasySummary` now unwraps the `rankings` field at the API boundary so the typed signature matches reality and every caller gets a flat array.

## v1.20.1 — 2026-04-19 (backend)

### Fixed

- Pulse 5-DAY sparkline was still rendering as one thick block after v1.20.0 when `min_date = playoff_start` clamped the window to ≤ 2 days. `get_team_sparklines_with_live` now keeps `min_date` as an SQL-side clamp only (avoiding pre-playoff scans of `daily_rankings`) but always returns a vector of exactly `days` entries — padded with zeros on the older-date edge. A team with a single scoring day now renders `[0, 0, 0, P, 0]` → five distinct bars regardless of when playoffs started.

## v1.20.0 — 2026-04-19 (backend) / v1.13.0 (frontend)

### Changed — Insights is now mirror-only

Every data path the Insights page depends on reads from the NHL mirror in Postgres. The request path makes zero NHL API calls apart from static URL construction; the cache-miss render no longer fans out to `api-web.nhle.com` and therefore cannot trigger a rate-limit cascade that poisons the daily prewarm. Split across four concerns:

- **Hot card**: top-20 season leaders come from `nhl_skater_season_stats`; L5 form for the top-20 comes from `list_player_form` (one SQL window-function scan). Previously 20 sequential `get_player_form` calls per cache miss.
- **Cold card**: rostered players' L5 form via `list_player_form` on the rostered set. Goalies are filtered out before the query. The probe call that previously fetched one player's game log just to detect whether playoff data existed is gone — an empty `list_player_form` result is the signal.
- **Today's Games**: schedule from `list_games_for_date`, standings context (streak + L10) from new `list_team_standings_context`, yesterday's result captions from the mirror's game rows, and pre-game matchup (leaders + goalies + team records) from `nhl_game_landing` via new `get_game_landing_matchup`. The meta poller now captures landing for newly-added FUT/PRE games on today's slate via `list_games_without_landing_for_date`, write-once guarded; the old `response_cache` path with the `insights_landing:{id}` key is retired.
- **Stanley Cup Odds**: bracket reads from `nhl_mirror::get_playoff_carousel`; team ratings (playoff Elo and the standings-blend) fed from `nhl_mirror::load_standings_payload`, which reconstructs the NHL-shaped JSON from typed rows so `compute_current_elo` and `team_ratings::from_standings` work unchanged.

### Added — nightly NHL Edge refresher

- `backend/src/infra/jobs/edge_refresher.rs`: pulls top-skating-speed and top-shot-speed telemetry for the top 30 season leaders, writes `nhl_skater_edge`. Sequential fetch paced at 500 ms between players (~15 s wall time). 18-hour freshness gate skips a run when a recent refresh already happened, so the 09:30 UTC cron and a nearby admin prewarm don't double up. The admin `/api/admin/prewarm` endpoint force-triggers a refresh before the insights pre-warm so the cached page carries the freshest Edge numbers.
- New migration `20260420010000_nhl_skater_edge.sql` creates `nhl_skater_edge(player_id PK, top_speed_mph, top_shot_speed_mph, updated_at)`.
- Hot card reads `nhl_skater_edge` via new `list_skater_edge`. Players the refresher hasn't covered yet render with blank speed tiles — preferable to blocking the page on a live fetch.

### Removed

- Regular-season Hot/Cold fallback when playoffs haven't started. Empty Hot/Cold lists now render a "playoffs haven't produced data yet" empty state; the Claude narrative for `hot_players` acknowledges the absence instead of inventing players.
- `InsightsSignals.hotColdIsRegularSeason` end-to-end — the flag is obsolete now that there's no RS fallback. The `HotPlayerCard` `isRegularSeason` prop and the conditional "season pts" / "playoff pts" label are gone; the label is now simply "playoff pts".

### Fixed

- Pulse League Live Board 5-DAY sparkline rendered as one solid full-width block for any team with a single scoring day. `Sparkbars` normalises bar height as `h = (v / max) × height`; with one data point `max == v` and the one bar fills the entire box. `get_team_sparklines_with_live` now zero-pads each team's vector against the full expected date sequence, so a team scoring only once returns `[0, 0, P, 0, 0]` and the component draws five distinct bars. Teams with zero total still return an empty vector and keep the grey baseline empty-state.

## v1.19.5 — 2026-04-19 (backend) / v1.12.2 (frontend)

### Changed

- Race-odds team table is now sorted by `projected_final_mean` descending, with `win_prob` as the tiebreaker and `team_name` as the final stable tiebreaker. Previously the sort key was `win_prob`, which is a Monte Carlo output frozen at the daily 10:00 UTC prewarm — so the table didn't re-rank as live scoring shifted projections during the day, and tied win-probability rows ordered randomly. Sorting on projected (which IS overlay-updated on every request via the v1.19.2 `current_points` shift) makes the rank reflect what's actually happening on the ice.
- LeagueRaceTable adds a footer caption explaining the data freshness split: `Current` and `Projected` update live on every request; `Win %` and `Top-3` come from the Monte Carlo last run at the timestamp shown. Reads `generatedAt` from the race-odds response and renders it as `HH:MM UTC today / yesterday / on YYYY-MM-DD`.

## v1.19.4 — 2026-04-19 (backend)

### Changed

- Skaters page (playoff branch) is now a real top-N points leaderboard rather than the eligible-roster pool. Source is `nhl_player_game_stats` aggregated per `player_id` via the new `nhl_mirror::list_top_skaters`, sorted by points desc / goals desc / id. Mirrors what `nhl.com/stats/skaters` shows. Goalies excluded. The fantasy-team-tag overlay is unchanged. Replaces the previous `playoff_roster_cache`-driven view that listed every rostered player whether they had skated or not.

## v1.19.3 — 2026-04-19 (backend)

### Fixed

- Skaters page (playoff branch) now shows real points/goals/assists per skater. The handler used to hard-code `points = 0` for every entry — it was treating the playoff endpoint as an eligible-roster lookup rather than a leaderboard, leftover from before there was anywhere to aggregate per-player playoff totals from. New `nhl_mirror::aggregate_skater_totals` sums each rostered player's rows in `nhl_player_game_stats` for the current `(season, game_type)`; the handler layers that onto the cached roster pool and sorts by points descending. Players with no games yet still show 0, which is correct.

## v1.19.2 — 2026-04-19 (backend)

### Fixed

- Pollers and rehydrate now use **Eastern Time** for "today" instead of UTC. NHL's `/schedule/{date}` endpoint keys games by ET local date — a 9 pm ET game on April 18 is in the response under `date = "2026-04-18"` even after the wall-clock at the server has rolled into UTC April 19. Previously the meta poller fetched `/schedule/2026-04-19` and filtered for that exact date, which silently dropped every late ET slate during the 4-hour window between midnight UTC and midnight ET — the mirror was empty for tonight's games even though the pollers were running on schedule. Same `Utc::now()` → `Utc::now().with_timezone(&America::New_York)` swap applied to `live_poller::tick_body` (so it queries the right `nhl_games` rows) and to `rehydrate` (so the playoff_start → today range is built in ET).
- Cold-start auto-seed: `main.rs` now spawns a one-shot rehydrate 45 s after boot if `nhl_player_game_stats` is empty. Eliminates the manual-`/api/admin/rehydrate`-after-deploy step that was needed for the v1.19.0 cutover.

## v1.19.1 — 2026-04-19 (backend) / v1.12.1 (frontend)

### Changed

- `scheduler::process_daily_rankings` reads finalised per-team totals from `v_daily_fantasy_totals` and upserts them into `daily_rankings` in one pass. The boxscore fan-out that was the last remaining NHL-API-per-game code path in a scheduled job is gone — the live poller already populates `nhl_player_game_stats` ahead of the 9am / 3pm UTC rollup, so the scheduler's job is now pure SQL. Preserves the old "skip if games still in progress" guard via `nhl_mirror::list_live_game_ids_for_date`.
- Pulse League Live Board: the `YESTERDAY` column is now `LATEST`. The underlying `points_today` field reads the most recent sparkline entry, which is today's running total during an active game day and yesterday's official rollup once the morning cron has fired. "Latest" covers both.
- `docs/DATA-PIPELINE-REDESIGN.md` status line flipped from "proposed — not yet implemented" to "shipped in v1.19.0"; the document now describes the architecture in production rather than a proposal.

## v1.19.0 — 2026-04-19 (backend) / v1.12.0 (frontend)

### Added — NHL mirror pipeline, layered architecture, prediction port

Every user-facing read path is now served from Postgres. Two background tasks continuously mirror the NHL API into `nhl_*` tables; handlers do not call `api-web.nhle.com` on the request path.

- `backend/src/infra/jobs/meta_poller.rs`: every 5 min, refreshes today's schedule in `nhl_games`. Every 6th tick (30 min) additionally refreshes tomorrow's schedule, the skater + goalie leaderboards (`nhl_skater_season_stats`, `nhl_goalie_season_stats`), standings (`nhl_standings`) and playoff carousel (`nhl_playoff_bracket`). Every 288th tick (24 h) refreshes team rosters (`nhl_team_rosters`). Each source is gated on the corresponding mirror table's `MAX(updated_at)` so a server restart doesn't re-fan every fetch when the data is already fresh.
- `backend/src/infra/jobs/live_poller.rs`: every 60 s, polls every game in today's slate whose state is `LIVE`/`CRIT`/`PRE`. Updates `nhl_games.home_score`/`away_score`/`game_state`/`period_*` and upserts every skater + goalie line in `nhl_player_game_stats`. When a game's state transitions `LIVE|CRIT → OFF|FINAL`, invalidates `pulse_narrative:{league}:*` for any league with rostered players in that game so the next Pulse visit regenerates its narrative with the final numbers.
- Both pollers acquire a `pg_advisory_lock` per tick, bound to a dedicated `PgConnection` for the lock's lifetime, so a multi-replica deployment only runs the work on one replica per tick.
- `GET /api/admin/rehydrate`: admin endpoint that runs every poller step synchronously plus a full boxscore backfill for every game in `nhl_games`. Paced (250 ms between roster fetches) and freshness-gated so repeat invocations are cheap.
- Cron schedules in `backend/src/tuning.rs` corrected to 6-field format (`0 0 <hour> * * *`). Previously the values were arranged as 5-field patterns with an extra trailing wildcard, which `tokio_cron_scheduler` parsed as "every hour at minute N" — the morning / afternoon rankings cron and the 10:00 UTC prewarm were firing 24× / 48× per day instead of daily.
- `backend/src/infra/db/nhl_mirror.rs`: typed repository for all eight mirror tables plus read helpers consumed by the rewritten handlers (`list_games_for_date`, `list_player_game_stats_for_games`, `list_player_form` window-function aggregation, `list_league_team_season_totals`, `sum_player_points`, freshness helpers, advisory-lock lifecycle).

Three ports define the swappable edges of the system; adapters live in `infra/`:

- `domain/ports/nhl_source.rs` — `NhlDataSource` trait (placeholder, production adapter is `infra/nhl/client::NhlClient`).
- `domain/ports/prediction.rs` — `PredictionService` trait. Production adapter `infra/prediction/claude::ClaudeNarrator` wraps Anthropic `/v1/messages`; fallback `NullNarrator` is wired when `ANTHROPIC_API_KEY` is unset so dev boxes without credentials still boot. `AppState` now carries `Arc<dyn PredictionService>`; the Pulse handler calls `state.prediction.pulse_narrative(...)` instead of building a Claude request inline.
- `domain/ports/draft_engine.rs` — `DraftEngine` trait (placeholder, production impl is the existing in-process WebSocket hub).

### Changed — layered architecture per Bulletproof Rust Web

Backend source regrouped into three architectural layers. No behaviour change in this migration commit; the moves preserve file history via `git mv`.

- `domain/` — pure business logic (no `axum` / `sqlx` / `reqwest`). Subtree: `models/` (moved from `src/models`), `ports/`, `services/` (moved from `src/utils/{nhl,fantasy}.rs`), `prediction/`.
- `infra/` — outbound-IO adapters. Subtree: `db/` (moved from `src/db`), `nhl/` (moved from `src/nhl_api` plus `src/utils/api.rs`), `jobs/` (moved from `src/utils/{scheduler,player_pool,playoff_ingest,historical_seed}.rs`), `prediction/{elo,claude}.rs`.
- `api/` — Axum handlers + DTOs + routes (unchanged shape).

### Changed — handlers read from the mirror

- `get_rankings` (overall), `get_daily_rankings`, `get_playoff_rankings`: SQL reads only. `get_rankings` and `get_playoff_rankings` now sum season totals from `nhl_player_game_stats` via `list_league_team_season_totals` rather than joining against the NHL stats-leaders leaderboard — the leaderboard only returns the top ~25 per category, so depth scorers contributed 0 to their fantasy team's total. Totals now match the per-day view.
- `list_games` (basic + extended) and `get_match_day`: read `nhl_games` + `nhl_player_game_stats` in batch queries. Form data (last 5 completed games per player) comes from `list_player_form`, a single window-function query. Playoff totals come from `list_player_playoff_totals`. No per-game, per-player NHL fan-out. Dropped cache keys: `games_extended:*`, `match_day:*`, `daily_rankings:*`.
- `get_pulse`: tiered cache. The live block (my team status, series forecast, my games tonight, league board) recomputes from the mirror on every request. Only the Claude narrative stays in `response_cache` under `pulse_narrative:*`; it's invalidated by the live poller on game-end transitions. The 5-day sparkline unions `daily_rankings` (finalized) with `v_daily_fantasy_totals` (today's live total) so the chart fills in on day 1 of a round instead of rendering blank.
- `generate_and_cache_race_odds`: cache hit path now overlays fresh `current_points` from the mirror and shifts each team's `projected_final_mean` / `p10` / `p90` by the per-team delta, so the Current column stays in lock-step with Rankings throughout the day. Monte Carlo outputs (win% / top-3% / likely range) remain point-in-time from the 10:00 UTC prewarm. Cache key bumped `race_odds:v3` → `v4`.

### Fixed

- Game scores were NULL in `nhl_games` for games finalized before the live poller first saw them LIVE. `upsert_boxscore_players` now derives `home_score` / `away_score` from the boxscore's skater + defense goals and writes them in the same transaction, so both live games and already-finalized ones converge to correct scores on the next poll or `/api/admin/rehydrate`.
- `GameState` now recognises `"PRE"` (pre-game warm-up). Previously the variant was missing so `nhl_games.game_state = 'UNKNOWN'` for any game NHL marked `PRE`, and `list_live_game_ids_for_date` skipped it because the filter requires `LIVE`/`CRIT`/`PRE`.
- Poller first ticks staggered (+15 s meta, +45 s live) and roster fetches paced (250 ms between calls) so server boot no longer produces a 429 cascade. Roster refresh tier (24 h) is freshness-gated so a restart within the TTL window is a no-op instead of a full 32-team re-pull.
- `pg_advisory_lock` acquire and release now happen on the same dedicated `PgConnection` — previously each query took whichever pool connection it got and the release silently failed against a different session, leaving the lock leaked until the holding session eventually cycled and emitting `you don't own a lock of type ExclusiveLock` NOTICEs.

### Changed — frontend automatic live refresh

- `useGamesData`: the opt-in "Auto-refresh" checkbox is gone. React Query's `refetchInterval` is wired to a predicate on the query result: poll every 30 s while any game on the selected date is `LIVE`/`CRIT`; stop otherwise. The Games page shows a passive "Live — auto-updating" badge when polling is active.
- `usePulse`: polls every `PULSE_STALE_MS` while the response's `hasLiveGames` is true; stops otherwise.
- `useRankingsData`: polls the daily rankings every 30 s while the selected date is today; historical dates are static.
- `frontend/src/components/games/GameOptions.tsx` deleted — it was the checkbox's only caller.

## v1.18.0 — 2026-04-18 (backend) / v1.11.0 (frontend)

### Fixed — rate-limit cascades during playoff slates

The playoff traffic pattern produced sustained NHL API 429 errors. Multiple devices loading `/games` or `/rankings` concurrently caused the same game IDs to appear in `NHL API rate limit exceeded after retries` errors every 2–4 minutes, with knock-on failures: Insights "Players to Watch" sidebar blank on games that went live before first generation, `/rankings/daily` returning 500s when any single boxscore 429'd, live game rows showing "just the time" and 0 pts for skaters whose points had already arrived.

Changes:

- `response_cache` row added for `GET /api/fantasy/rankings/daily` (`daily_rankings:*`). Previously the handler re-fanned N boxscore calls per request.
- Per-game pre-game landing cache (`insights_landing:{game_id}`) with write-once semantics. First successful FUT-state fetch locks in the matchup block; later LIVE-state fetches never overwrite.
- Playoff roster pool persisted to Postgres (`playoff_roster_cache`, new migration `20260419000000_playoff_roster_cache.sql`). Refreshed by the 10:00 UTC prewarm. Replaces the 16-team parallel `try_join_all` fanout on cold `/stats` or `/draft` hits.
- Games extended-mode: retry failed live-game boxscores once sequentially; derive `home_score`/`away_score` from the boxscore when schedule + `get_game_scores` both return null.
- NhlClient retry budget: 3 linear retries (max 1.5 s) → 5 exponential retries (500 ms → 8 s, ~15 s total).
- Insights cache write gate removed. The previous "only cache when every game's landing succeeded" rule meant one rate-limited landing killed caching for the entire day, so every visitor re-ran the full signal compute plus the Claude call.
- Insights response cache self-heals only on an empty-schedule response; partial-landing responses are now cached and served for the day.
- Frontend rankings widget defaults to yesterday via new `getMostRecentRankingsDate()`. The previous default (today) always showed "No daily rankings available for this date" during live slates.

### Added — centralised tuning module and data-pipeline plan

- `backend/src/tuning.rs`: every timeout, retry count, cron schedule, cache TTL, and (reserved) poller cadence the service uses. Grouped by subsystem (`nhl_client`, `scheduler`, `http`, `live_mirror`) with per-constant rationale. Replaces scattered literals in `nhl_api/nhl.rs`, `utils/scheduler.rs`, `main.rs`, `api/mod.rs`, `api/handlers/insights.rs`, `api/handlers/pulse.rs`, and `ws/handler.rs`.
- `frontend/src/config.ts`: new `QUERY_INTERVALS` object centralises React Query `staleTime`, the Games-page auto-refresh interval, Pulse / Insights / Race-Odds per-hook overrides, and the draft-room elapsed-time tick. Call sites updated in `lib/react-query.ts`, `features/games/hooks/use-games-data.ts`, `features/insights/hooks/use-insights.ts`, `features/race-odds/hooks/use-race-odds.ts`, `features/pulse/hooks/use-pulse.ts`, and `pages/DraftPage.tsx`.
- `TECHNICAL-CACHING.md` at repo root: current caching architecture (two cache layers, per-endpoint flow, data freshness table, scheduled jobs, frontend refresh patterns, rate-limit offenders and post-fix status).
- `docs/DATA-PIPELINE-REDESIGN.md`: proposed follow-on redesign — NHL mirror tables, metadata + live pollers, pure DB reads in every handler, 60-second live update cadence flowing through Pulse / Rankings / Stats / Sleepers. Migration `20260420000000_nhl_mirror.sql` lands now; the pollers and handler rewrites are a separate PR.

## v1.17.0 — 2026-04-18

### Changed — halved Games-page cold-load time

Two compounding waits killed the cold-load "Loading games data…" spinner on playoff nights:

1. The NhlClient semaphore capped parallel NHL API calls at **5**. On a night with ~60–100 unique rostered skaters to fetch game logs for, that meant ~12 sequential batches at ~400ms each = 5–10 seconds of wall time just for the prefetch.
2. Inside `process_games_extended`, boxscore prefetch and player-game-log prefetch ran **sequentially** — boxscores first (awaited to completion), then game logs. The shorter job gated the longer one.

**Fixes:**
- Bump the NhlClient semaphore `5 → 10`. NHL API tolerates 10 comfortably in practice; the existing 429 retry handles the rare overshoot.
- Fuse the two prefetch groups into a single `tokio::join!` so they fire concurrently. Removed the duplicate `prefetch` block that was re-running the game-log fetch after the boxscore wait.

Expected cold-load impact: ~2× faster on playoff-night slates with full fantasy rosters. Warm-cache paths are unchanged.

## v1.16.0 — 2026-04-18

### Changed — Elo seeding now applies 0.7 production shrinkage

The v1.13.0 sweep harness was exercised against all four backfilled seasons (2021-22 through 2024-25) with a 6-cell grid varying `points_scale ∈ {3, 4, 6}` and `shrinkage ∈ {0.7, 1.0}`. Aggregate Brier per config averaged across the four seasons:

| Knobs | Avg brAgg |
|---|---|
| `ps=6, sh=0.7` | **0.5386** |
| `ps=4, sh=1.0` | 0.5390 |
| `ps=6, sh=1.0` (v1.15 production) | 0.5424 |
| `ps=3, sh=1.0` | 0.5437 |
| `ps=4, sh=0.7` | 0.5473 |
| `ps=3, sh=0.7` | 0.5538 |

The winner beats the legacy defaults across-the-board and is notably more stable on 2022-23 (the BOS-R1-upset season, where `sh=1.0` produced a 0.72 Brier outlier). New `playoff_elo::PRODUCTION_SHRINKAGE = 0.7` constant is now applied by the production `seed_from_standings` wrapper. `seed_from_standings_tuned` still lets callers (calibration sweep) override both knobs.

User-visible effect: Stanley Cup Odds top-seed probabilities come in materially lower. A +200-Elo favourite no longer compounds to 94% R1 / 39% Cup — after shrinkage and the existing round-depth mean reversion, the same team lands closer to ~82% R1 / ~28% Cup. Still favoured, not anointed.

Cache key bumped `race_odds:v2` → `race_odds:v3` so any cached pre-shrinkage payload from today's pre-warm is regenerated on the next request.

## frontend v1.10.2 — 2026-04-18

### Removed — Pulse 30s auto-refresh

`usePulse` was polling every 30 seconds while a game was live, and `PulsePage` rendered a red "LIVE — AUTO-REFRESHING EVERY 30S" banner to tell the user about it. Playoff data does not change quickly enough to justify that cadence, and v1.15's richer race-odds path means every Pulse refresh now does a heavier server-side sim. Removed both the `setInterval` and the banner. React Query's default `refetchOnWindowFocus` still produces a fresh fetch when the user returns to the tab; `staleTime` bumped from 15s → 60s to match.

Games-page auto-refresh (same 30s cadence) is untouched — that's the box-score view where mid-period score changes genuinely matter.

## frontend v1.10.1 — 2026-04-18

### Fixed — stale Stanley Cup Odds methodology blurb

The paragraph above the Cup Odds table still claimed the model "underweights goalie quality and injuries" and was "calibrated against HockeyStats.com round-1 reference odds within ~3pp". Both became false in v1.15.0: the goalie component is now a first-class `TeamRating` field, and the HockeyStats calibration referenced an obsolete `DEFAULT_K_FACTOR` tuning from v1.7 for the pre-playoff standings path, not the current Elo engine. Replaced with a terse, accurate description of what the engine actually does now (standings Elo + dynamic playoff replay + starter SV% + home/road split + round-depth mean reversion).

## v1.15.0 — 2026-04-18

### Added — goalie-strength component on TeamRating

The team-rating model was a single scalar plus a home-ice delta — a .930 starter and a .895 starter produced identical pre-sigmoid gaps when their standings points matched. v1.15 adds `goalie_bonus` as a third component, symmetric around zero.

New pure-domain module `domain::prediction::goalie_rating`:
- `GoalieEntry { player_id, team_abbrev, wins, save_pct }` — a framework-free projection of the NHL API goalie leaderboard.
- `compute_bonuses(entries)` picks each team's primary starter (most wins, ≥ 3 wins) and maps `(sv_pct - 0.905) × 800` clamped to `±30` Elo. Tandems (two goalies within 3 wins) average their bonuses.
- `bonus_for_svp` exposed for unit tests.

`TeamRating` gains `goalie_bonus: f32` with a chainable `with_goalie_bonus` builder. `simulate_series` is unchanged — the caller folds the bonus into the rating gap before passing it in. Goalie contribution applies at full weight for live (`InProgress`) series and shrinks with `round_depth_shrinkage` for `Future` slots on the theory that starters rotate and get hurt deeper into the bracket.

### Added — round-depth mean reversion for Future bracket slots

`race_sim::run` was using the same rating gap for a first-round matchup and a hypothetical Cup Final between two projected winners three rounds out. The gap was uniformly wide, which compounded confidence in the better-seeded team through the bracket.

New `round_depth_shrinkage(round_idx)`:
- Round 0 (current): 1.00 — unchanged.
- Round 1 (conference semis): 0.85.
- Round 2 (conference finals): 0.70.
- Round 3 (Cup Final): 0.55.

Only applied to `Future` slots. `InProgress` and `Completed` states have known participants and pass through unchanged. Combined effect across the bracket tree: a +200 Elo favourite still looks like a significant favourite in round 1, but their Cup-win probability no longer compounds as if the same 200-Elo gap applies to a hypothetical survivor matchup in round 4.

### Changed — NHL client now fetches goalie leaderboards

`NhlClient::get_goalie_stats(season, game_type)` calls `/v1/goalie-stats-leaders/{season}/{game_type}` (present in `nhl_constants` since v1.7 but previously unused). The race-odds handler and calibration path both pull regular-season (`game_type = 2`) goalie data — playoff SV% is too small a sample and circularly part of what we're predicting.

### Changed — calibration now scores the full v1.15 model

`infra::calibrate::build_ratings` accepts a pre-computed goalie-bonus map; `calibrate_season_with_knobs` fetches the historical season's goalie leaderboard via the same pure-domain module. Grid-search results now reflect production behavior instead of an Elo-only model that's missing ~25% of the strength signal.

## v1.14.0 — 2026-04-18

### Changed — player projection now uses shots and TOI, not just points

`infra::prediction::project_players` was selecting only `(player_id, points)` from `playoff_skater_game_stats`, throwing away the `goals`, `assists`, `shots`, `pp_points`, and `toi_seconds` columns the ingest has been populating for months. `project_one` now consumes a `&[GameStats]` carrying all six.

Two new signals feed the projection:

- **Shot-volume stabilisation of goal rate.** Observed playoff goals-per-game is blended 60/40 with `shots_per_game × LEAGUE_SH_PCT` (0.095) when shot data is available. A high-volume shooter with zero goals over three games used to project 0 goals/game until the points-blend's RS prior eventually pulled them back up. Now they regress toward expected finishing inside two or three games. Symmetric pull: a shooter going 3-for-3 on shots gets regressed down instead of sustaining a 100% shooting-pct projection.
- **TOI-ratio lineup multiplier.** After ≥ 3 recent + 3 older games with non-null `toi_seconds`, the ratio `recent_avg / older_avg` clamps to `[0.70, 1.10]` and multiplies the final PPG. A 4th-line demotion (18 min → 9 min) now derates projections 30%; a 3rd-pair → 1st-pair promotion caps at +10% (asymmetric because one high-TOI overtime game can fake a promotion signal). Exposed on `Projection.toi_multiplier` for future UI badges.

Blend shape (ALPHA/BETA Bayesian weights, recency half-life, availability multiplier) unchanged. `Projection` gains `toi_multiplier: f32` alongside the existing `ppg` and `active_prob`; additive, not breaking.

Not in this release (deliberate scope cut): formal split of `rs_goals`/`rs_assists` from total `rs_points` in `PlayerInput`. The `StatsLeaders` feed the crate consumes only exposes category leaderboards (top-N per stat), not per-player breakdowns — adding this would require cross-referencing the `goals` and `assists` categories plus a league-fraction fallback for non-top-N skaters. Shot-stabilisation above captures the dominant signal for skater-level improvement without touching the ingest.

## v1.13.0 — 2026-04-18

### Added — calibration sweep harness

New admin endpoint `GET /api/admin/calibrate-sweep` runs `calibrate_season` over a grid of hyperparameter combinations and ranks by aggregate Brier. Comma-separated lists on `points_scale`, `shrinkage`, `k_factor`, `home_ice_elo`, and `trials`; the endpoint takes the Cartesian product (capped at 200 cells). Each run is deterministic — `simulate_with_seed` with a fixed RNG seed means Brier deltas between grid cells come entirely from knob changes, not Monte Carlo noise.

`infra::calibrate::CalibrationKnobs` now carries every tunable the sweep explores. `calibrate_season` is a thin wrapper over `calibrate_season_with_knobs(_, CalibrationKnobs::default())` so existing admin callers are unaffected. Production constants (`POINTS_SCALE = 6.0`, shrinkage 1.0, live `k_factor`, `HOME_ICE_ELO = 35.0`) are unchanged in v1.13.0 — this release ships the wrench, not the tuning pass.

### Changed — playoff Elo seeding accepts a shrinkage factor

`playoff_elo::seed_from_standings_tuned(standings, points_scale, shrinkage)` is the new entry point the sweep calls. `shrinkage ∈ [0, 1]` regresses each team's RS-point deviation from league average toward the mean before scaling, so the sweep can test "current standings are 70% signal, 30% noise" without touching production.

`playoff_elo::seed_from_standings(standings)` remains the default-knobs path and its output is identical to v1.12.3.

### Changed — per-team home-ice bonus is centered on HOME_ICE_ADV

`home_bonus_from_standings` previously returned a raw Elo value clamped to `[10, 80]`. A team with a league-average home/road split earned raw ≈ 35 and landed inside the band correctly, but a team with a truly neutral split (raw 0) clamped to the floor of 10 — materially less home-ice than a league-average team, which was not the intent.

The function now computes the delta from `HOME_ICE_ADV` (35 Elo, the league baseline), clamps that delta to `±HOME_BONUS_DELTA_CLAMP = 15`, and returns `HOME_ICE_ADV + delta`. Output range is `[20, 50]`. A hot-home / cold-road team earns up to 50; a genuinely flat team earns ~35; a weak-at-home team can drop to 20. No team is locked into 10 anymore.

## v1.12.3 — 2026-04-18

### Fixed — calibration now seeds Elo from historical, not current, standings

Previously `/api/admin/calibrate` computed Elo seeds from `NhlClient::get_standings_raw()` — the *live* standings snapshot. Scoring a past season's bracket with current-roster ratings confounded the Brier numbers: the model's "team strength" signal was based on 2026 rosters even when predicting 2023 outcomes. R1 Brier came in around 0.35 across four past seasons — worse than the 0.25 coinflip baseline — not because the model structure was bad but because its inputs were.

New `NhlClient::get_standings_for_date(date)` hits `/v1/standings/{YYYY-MM-DD}`. New `infra::calibrate::fetch_historical_standings` walks back day-by-day from the season's first playoff game date looking for a non-empty standings response (the NHL endpoint returns an empty array for dates in the gap between the regular-season finale and playoff game 1, so a fixed "day before" isn't enough). Up to 10 tries, then falls back to live standings with a warn log.

Side-effect: the per-team home-ice bonus (derived from `homeWins`/`homeLosses`/`homeOtLosses` via `playoff_elo::home_bonus_from_standings`) is now also season-accurate instead of reflecting the current season's home/road split.

### Fixed — rebackfill URL used the wrong season format

`/api/admin/rebackfill-carousel` was constructing URLs like `/v1/schedule/playoff-series/2023/a` using the 4-digit calendar year, which returns 404 from the NHL API. The endpoint actually requires the full 8-digit season (`/v1/schedule/playoff-series/20222023/a`). Every series fetch 404'd, every season reported 0 rows. The v1.12.1 error-surfacing exposed the 404 responses in the logs, which is how the bug was diagnosed.

Removed the `short_year` derivation from the admin handler and the `rebackfill_playoff_season_via_carousel` signature; the endpoint now passes the 8-digit season through unchanged.

## v1.12.1 — 2026-04-18

### Fixed — Pulse "No games scheduled today" stuck after schedule landed

Same bug pattern I already fixed in Insights in v1.10.0: Pulse caches the response by hockey-date. If the morning prewarm (or any early visit) ran before the NHL schedule was published, the cached response has `has_games_today: false` and sticks all day. Today's Pulse incorrectly showed "NO GAMES SCHEDULED TODAY" and the narrative referenced no-games-today even while the Games page showed 3 real games.

**Fix**: on cache hit, if `cached.has_games_today` is false, throw it out and regenerate. On store, only cache responses with `has_games_today: true` — empty-schedule responses regenerate cheaply each visit (the NhlClient caches the upstream schedule fetch anyway).

### Changed — rebackfill surfaces errors instead of swallowing them

v1.12.0's `/api/admin/rebackfill-carousel` silently logged-and-continued on any `get_playoff_series_games` failure, making the response `"Rebackfilled 0 ..."` with no indication of what went wrong. The handler now:
- Propagates the first series-fetch error as a 500 with the real NHL-side message (rate limit, JSON parse failure, HTTP error).
- Emits per-series diagnostics to the backend log including the game-count in the feed, the count accepted, and per-game skip reasons (`not_completed`, `no_score`, `no_start_time`).

### Added — carousel-driven re-backfill for historical playoffs

New admin endpoint `GET /api/admin/rebackfill-carousel?season=20222023`. Walks the playoff carousel + `/v1/schedule/playoff-series/{short_year}/{letter}` for each series and upserts every completed game's team-level row into `playoff_game_results`. This bypasses the date-iteration schedule endpoint that dropped Cup Finals and conference-finals games when queried retroactively.

**Why this endpoint is better for historical data**: the `/schedule/{date}` response for a date in 2023 doesn't reliably include the playoff games that happened that day — the NHL API's retroactive `series_status` population is spotty. `/schedule/playoff-series/{year}/{letter}` is keyed by series, not by date, and reliably returns every game in that series with ID, start-time, home/away teams, scores, and game state. One call per series × 15 series per season = 75 calls for 5 historical seasons. Fast, clean, idempotent.

**Scope**: team-level only — populates `playoff_game_results`. Skater-level data for past seasons (`playoff_skater_game_stats`) is not written here because (a) the per-game-boxscore fetch multiplies the call count by ~40× and (b) the current `player_projection` module only reads the current season's skater game log, so historical skater stats don't unblock anything immediate. A future pass can layer skater ingest on top of the same series walker if needed.

**New types**: `PlayoffSeriesGames`, `PlayoffSeriesGame`, `PlayoffSeriesTeam` in `models::nhl`, plus `NhlClient::get_playoff_series_games(season, letter)`. `GameState` now derives `Default` (Unknown).

## v1.11.1 — 2026-04-18

### Fixed — calibration scoring vs corrupt realized outcomes

v1.11.0's `/api/admin/calibrate` produced Brier scores worse than coinflip because the realized-outcome reconstruction couldn't find Cup winners or late-round advancements. Root cause: NHL's schedule endpoint returns `series_status.round` inconsistently for historical games. The 2022-23 Cup Final had 0 round-4 rows in the DB; same for 2023-24 and 2024-25. Conference-final games dropped into R1's 8-slot bucket where they got truncated.

**Fix**: `reconstruct_bracket_from_results` (`domain/prediction/backtest.rs`) no longer consults the `round` column at all. Rounds are inferred topologically:

1. Group all games into series by canonical team-pair.
2. **R1** = first series in date order that *introduce* at least one new team. Cap at 8. (First-game-date alone fails when there are few series total; the "introduces a new team" heuristic correctly distinguishes R1 matchups from rematches in later rounds.)
3. **R2** = next up to 4 series where *both* participants are R1 winners.
4. **R3** = next up to 2 series where both participants are R2 winners.
5. **Cup Final** = next 1 series where both participants are R3 winners.

`ResultRow` grew a `game_date: String` field so the reconstruction can sort chronologically. The `round` field is kept but explicitly documented as ignored. Two new tests cover the topology-only path: one asserting a lying `round = 99` on R2 rows is ignored, one asserting Cup Final falls out of R3 winners across a full 16-team bracket.

**Still expected**: Cup Final data is genuinely missing from the DB for 4 of 5 backfilled seasons — the NHL schedule endpoint didn't return those games during the original backfill. Fix A can't recover rows that aren't there; a separate re-backfill path via the `playoff-series/carousel/{season}` endpoint is queued as a follow-up.

### Added — calibration admin endpoint (P4.2 MVP)

New `GET /api/admin/calibrate?season=20222023` endpoint measures how calibrated the current race-odds model is against a completed historical season's realized outcomes. Requires the season to be backfilled first via `/api/admin/backfill-historical`.

How it works (`backend/src/infra/calibrate.rs`):
1. Loads every completed game for the requested season from `playoff_game_results`.
2. Folds the games through `backtest::reconstruct_bracket_from_results` to get the realized bracket — who actually advanced out of each round.
3. Rebuilds the day-1 `BracketState` (same R1 pairings, wins reset to 0-0, later rounds `Future`).
4. Seeds Elo from the NHL standings endpoint, applies the current production hyperparameters (`ELO_K_FACTOR`, home-ice bonus, NB dispersion), runs 5000 Monte Carlo trials.
5. Scores the predicted round-advancement probabilities against the `{0, 1}` realized outcomes per team with **Brier score** and **log-loss**, per round (R1 / CF / Cup Final / Cup).

The response includes per-team `predicted_advance_r1 / _r2 / _r3 / _cup_win` alongside the realized outcome, plus aggregate metrics per round. A reasonable Brier on R1 is ≤ 0.22 (the "always predict 50%" baseline is 0.25, a perfect model is 0); the further out you look, the higher the irreducible noise — Cup-winner Brier near 1/16 × 15/16 ≈ 0.06 is already "can't really do better."

This is the minimum-viable instrument. If one-season Brier looks off, a follow-up lands a grid-search tuning pass; if not, tuning is low-leverage and we move on.

**Caveat**: the MVP seeds Elo from the *current* standings snapshot (NHL API doesn't expose historical standings at the right date without extra plumbing). For the most recently completed season the bias is small; for 2021 it's larger. Worth remembering when interpreting the numbers.

### Added — per-team home-ice advantage

Replaced the league-constant `HOME_ICE_ELO = 35` with per-team values derived from each team's regular-season home-vs-road record. `TeamRating` expanded from a scalar tuple struct to `{ base, home_bonus }`; both are on the Elo scale. `playoff_elo::home_bonus_from_standings` parses `homeWins/homeLosses/homeOtLosses` vs `roadWins/…` from the standings feed, computes `(home_pts_pct − road_pts_pct) × 400`, and clamps to `[10, 80]` to smooth small-sample noise. `simulate_series` prefers the home team's own `home_bonus` when non-zero; otherwise falls through to the league-wide `input.home_ice_bonus`. Pre-playoff path unchanged (standings-points scale, bonus=0). Test coverage extended to 54 passing.

### Added — historical playoff game-results backfill

New admin endpoint `GET /api/admin/backfill-historical?start=YYYY-MM-DD&end=YYYY-MM-DD` calls the existing `ingest_playoff_games_for_range` across a date range, upserting completed `game_type == 3` games into `playoff_game_results` and `playoff_skater_game_stats`. Meant to be run once per past season to seed the training data needed for P4.2 hyperparameter tuning. Idempotent.

### Fixed — Insights narrative "No games on the slate today"

The insights response was cached for the whole hockey-date. If the 10am UTC prewarm ran before NHL published today's schedule, the stale narrative stuck all day. Now the handler self-heals: on cache hit, if `todays_games.is_empty()`, the cached entry is thrown out and regenerated. Likewise, empty-schedule responses are no longer cached — off-days regenerate cheaply on each visit (the NhlClient still caches the upstream schedule response) rather than committing a misleading narrative.

### Added — dashboard quick-link buttons

`ActionButtons` dropped the old "View All Teams / Game Center / View Full Rankings" trio in favour of four targets: **Pulse**, **Insights**, **Today's Games**, **Detailed Stats**. League-scoped where applicable; the Games link is global.

### Changed — Pulse cleanup

- **Removed the standalone "Head-to-Head" bar** between the narrative and Race Odds. The Race Odds section already shows the same rivalry card at its top; having both was redundant.
- **Stanley Cup Odds: dropped the `YOU: N` pill** from rostered-team cells. Ownership info is redundant on the NHL-centric Insights surface.
- **Games page: "Show Game Details" → "Show Rostered Skaters"**. The toggle reveals fantasy-team skaters active in the NHL game; old label implied game-level info.

## v1.9.0 — 2026-04-18

### Refactored — prediction engine isolated in `domain/prediction/`

The race-odds Monte Carlo, playoff Elo, player-projection blend, team ratings, series-state classifier, and backtest helpers now live under `backend/src/domain/prediction/` as pure-domain code with zero framework dependencies (no `sqlx`, no `axum`, no `reqwest`). The two DB-backed wrappers that used to live mixed-in — `compute_current_elo` and `project_players` — moved to `backend/src/infra/prediction.rs` and call into the pure domain helpers. Aligns the backend with the layered shape from [bulletproof-rust-web](https://github.com/gruberb/bulletproof-rust-web); sets up the extraction work sketched in `PREDICTION_SERVICE.md` at the repo root, which lays out how to lift the engine into a standalone crate or HTTP service later for re-use across products (e.g. a prediction-market frontend).

### Fixed — Games page loading latency

`/api/games?detail=extended` was serial end-to-end on cold loads: box-scores fetched in a for-loop one at a time, then per-team / per-player `get_player_game_log` calls also sequential inside the loop. On a 16-game slate with ~20 rostered players per team, that's ~640 sequential NHL round-trips. Fixed:

- **Box-score pre-load now parallel** via `join_all`. The NhlClient's 5-concurrent semaphore still throttles, so we don't burst NHL; we just stop serializing when we don't need to.
- **Pre-warm player-game-log cache** by firing every rostered skater's `get_player_game_log` in parallel before the sequential post-processing runs. The serial calls downstream then hit the in-memory cache instead of doing network. ~640 serial → ~130 parallel → 5 concurrent at the NHL boundary.

Cold-load "Loading Games Data…" falls from multi-second to sub-second on most date navigations.

### Fixed — iOS team names truncating to empty

**Pulse > League Live Board**: the grid was sized `[2rem_1fr_4rem_4rem_4rem_5rem]` regardless of viewport. On a 375px iPhone minus padding/gaps the fixed tracks summed to more than the viewport width, so the `1fr` Team column collapsed to zero. Now uses a tighter `[1.5rem_minmax(0,1fr)_2.5rem_2.5rem_2.5rem]` on mobile (5-day sparkline column hidden), widens to the full 6-track layout at `sm:`. "Yesterday" column header also shortens to `Y'day` on mobile.

**Insights > Stanley Cup Odds**: team rows rendered `getNHLTeamShortName(abbrev)` (`HURRICANES`, `PANTHERS`, …) into a narrow truncating cell that on iPhone clipped to near-nothing. Mobile now shows the 3-letter `abbrev` (`CAR`, `FLA`); desktop keeps the long name.

### Added — PREDICTION_SERVICE.md

Plan doc at the repo root for extracting the prediction engine into a standalone crate or HTTP service. Covers current state, motivations (reuse, independent scaling, calibration isolation), three architecture options (workspace crate / standalone HTTP / gRPC), the JSON data contract for `/simulate`, and a phased migration path.

## v1.8.1 — 2026-04-18

### Fixed

- **Auto-apply migrations on startup.** v1.8.0 shipped three new tables (`playoff_skater_game_stats`, `historical_playoff_skater_totals`, `playoff_game_results`) but Fly deploys don't run Supabase migrations. The server booted against a DB missing those tables and the race-odds / backfill paths errored with `relation does not exist`. Now `main.rs` runs `sqlx::migrate!("./supabase/migrations")` at boot, embedding the `.sql` files into the binary at compile time and tracking applied versions in `_sqlx_migrations`. Every one of the existing migrations uses `CREATE ... IF NOT EXISTS` / `DO $$` guards, so the coexisting Supabase-CLI tracker and this sqlx tracker don't fight — sqlx re-"applies" prior migrations as no-ops on first boot, then becomes authoritative going forward.
- **Tonight player rows: overlapping team label.** v1.8.0 used `getNHLTeamShortName(p.nhlTeam)` for the per-player team tag inside Tonight game cards, which returned long names ("HURRICANES") that overran the 2rem column and stacked behind the player name. Now uses `p.nhlTeam` directly (the 3-letter abbrev), widened the column to 2.25rem, and added `min-w-0` on the name anchor so the truncate works.

## v1.8.0 — 2026-04-18

Race-odds rework: the Monte Carlo engine was sound but the inputs it ran on were blunt. This release restructures the sim to be correct end-to-end across the bracket, switches from frozen-RS team strength to a game-log-driven playoff Elo, replaces the crude `rs_points/82` PPG with a Bayesian blend that leans on a real playoff history, and widens the per-player tails with a Negative-Binomial draw.

### Fixed — bracket-state correctness

The sim was only bracket-state-aware for round 1. Once round 2 starts (~Apr 29), the old `RaceSimInput { round1: Vec<CurrentSeries>, ... }` + `pair_and_simulate(from 0-0)` path would have re-opened partially- or fully-decided R2+ series while `games_played_from_carousel` was already summing R2+ games into `games_played_so_far`. A team up 3-0 in R2 would still be simulated as ~50/50 to advance, and the fantasy-points Poisson draw would see an inconsistent `remaining = team_games - already_played` that silently saturated to zero. Invisible on day 1 of playoffs; would have corrupted projections mid-second-round.

- **New `SeriesState` enum** in `backend/src/utils/race_sim.rs`: `Future | InProgress { top_team, top_wins, bottom_team, bottom_wins } | Completed { winner, loser, total_games }`. Every slot in the bracket is tagged and the sim resolves each differently per trial.
- **New `BracketState` struct** — full playoff tree as `rounds: Vec<Vec<SeriesState>>`, positional pairing (`rounds[r+1][i]` fed by `rounds[r][2i]` and `rounds[r][2i+1]`).
- **`RaceSimInput.round1` → `RaceSimInput.bracket`**. `games_played_so_far` dropped from the sim's input; per-trial tracking is now `remaining_games` only.
- **`bracket_from_carousel`** in `backend/src/api/handlers/race_odds.rs` walks all four rounds of the NHL carousel and pads missing slots with `Future`.
- **`expected_games` semantics preserved** as "average total games across the run" = `already_played + mean(remaining)`.

### Added — persisted playoff facts

The input pipeline can't improve without per-game data. Two new tables plus a nightly ingest:

- **`playoff_skater_game_stats`** — one row per `(game_id, player_id)` with goals, assists, points, shots, pp_points, team, opponent, home flag.
- **`playoff_game_results`** — one row per `game_id` with team scores, winner, round. Chronological-replay index for the Elo update loop.
- **`utils/playoff_ingest`** — nightly ingest of yesterday's completed box scores (10am UTC scheduler step, before the existing insights + race-odds prewarm). Same module handles the startup backfill from `playoff_start` → today if the table is empty. Goalies skipped. Upsert-on-conflict keeps re-runs idempotent.

### Added — 5-year historical seed

- **`historical_playoff_skater_totals`** table (keyed `(player_name, born)` to disambiguate the real-world duplicate "Sebastian Aho").
- **`backend/scripts/parse_historical_playoff_skaters.py`** parses the tab-separated hockey-reference export. Handles TOT (traded) rows that split across two physical lines and drops repeated-header artifacts. Output: 600 rows, ~36 KB CSV at `backend/data/historical_playoff_skater_totals.csv`.
- **`utils/historical_seed`** embeds the CSV with `include_str!` so the Fly binary stays self-contained, then runs an idempotent UNNEST-driven bulk INSERT once at startup.

### Added — dynamic playoff Elo

Replaces `team_ratings::from_standings` as the sim's team-strength source when `game_type == 3`:

- **`utils/playoff_elo`** — seeds `elo_0 = 1500 + 6·(season_points − league_avg)` from the NHL standings, replays every completed playoff game chronologically with the standard logistic-Elo update, `+35` home-ice advantage, and a `ln(|goal_diff|+1)` blowout multiplier. K=6. Missing-team policy falls back to last persisted rating (or surfaces the failure rather than silently flattening at 0.0).
- **New `ELO_K_FACTOR = ln(10)/400 ≈ 0.00576`** in `race_odds.rs` for the Elo rating scale. Applying the old `k = 0.010` (tuned for the RS-points scale) to Elo would pin every series outcome to the favorite.

### Added — Bayesian player projection

- **`utils/player_projection`** — replaces `race_odds::player_ppg` when on the playoff path. Shrinkage blend of four signals:
  ```
  projected_ppg = (α·rs_ppg + po_gp·blended_po_ppg + β·hist_ppg) / (α + po_gp + β)
  blended_po_ppg = 0.65·po_ppg + 0.35·recent_ppg
  recent_ppg     = Σ 2^(−i/4) · points_i / Σ 2^(−i/4)   over the last 10 team games
  ```
  `α = 10`, `β = 4` (games-equivalent prior strengths). Historical prior resolved by name match. Availability multiplier `0.3` mutes a player who's absent from all playoff games after their team has played ≥3.
- **`project_players` loads every rostered skater's signals in one DB round-trip.** `build_fantasy_teams_playoff` (new in `race_odds.rs`) flattens across fantasy teams, batches the query, and assembles `SimFantasyTeam` off the returned map. Same path for `build_champion_input`'s top-40 global leaderboard.

### Changed — simulation polish

- **Negative-Binomial sampling** replaces plain Poisson for per-player point draws. Gamma-Poisson mixture with dispersion `r = 4`: variance `= λ + λ²/r`. Bridges the "Poisson is too tight" gap in `p10/p90`, head-to-head pairwise, and top-3 tails. Mean unchanged. `DEFAULT_K_FACTOR` / `ELO_K_FACTOR` independent, so upstream calibration is not affected.
- **Fractional tie-splitting in `win_prob` / `top3_prob`**. Teams tied for first now share `1 / tied_count` each. Top-3 credit splits at the rank-3 boundary. Ensures `Σ win_prob ≈ 1.0` in small leagues where ties are possible; previously one team got full credit by sort order.
- **Model version in cache key**: `race_odds:v2:...` so deploys don't serve stale same-day odds under the old model.

### Added — backtest scaffolding

- **`utils/backtest`** exposes `brier_score`, `log_loss`, `calibration_curve`, `mae`, `rmse`, `interval_coverage`, and `reconstruct_bracket_from_results` (group completed games into series, pad missing slots with `Future`). Enough to measure calibration against realized outcomes once enough games accrue. A full historical-day simulation loop is the next step but is gated on ingesting 2021-2025 box scores into `playoff_game_results`.

### Added — forward home-ice in the sim

The forward Monte Carlo had been running every series as a neutral-site set. Now `simulate_series` threads a pre-sigmoid `home_ice_bonus` through the per-game draw, stepping through the NHL 2-2-1-1-1 schedule so games 1, 2, 5, 7 favor the home-ice-owning team and 3, 4, 6 favor the road team. Home-ice ownership: InProgress slots honor the carousel's top seed (higher RS seed); Future slots award home-ice to the winner with the higher rating (proxy for RS standings). On the Elo path, the bonus is `ELO_K_FACTOR × HOME_ICE_ELO` with `HOME_ICE_ELO = 35` — the same 54/46-home-split constant the Elo replay already uses. Pre-playoff path passes `home_ice_bonus = 0.0` so behaviour is unchanged.

### Changed — Insights Playoff Bracket Tree uses playoff Elo

`backend/src/api/handlers/insights.rs` was calling `team_ratings::from_standings` for the STRENGTH labels on the Playoff Bracket Tree, which froze at RS values once the L10 window closed. Now during `game_type == 3` it reads `playoff_elo::compute_current_elo` instead, so the bracket tree and the Stanley Cup Odds table on the same page agree on which team is stronger. Pre-playoff path still uses the blended standings rating.

### Fixed — Pulse pre-drop state

- **Sparkline clipped at playoff_start.** `get_team_sparklines` now takes a `min_date` floor and Pulse passes `playoff_start()` so regular-season remnants in `daily_rankings` never surface as "Yesterday" points on day 1 of a new round. On day 1 every team correctly shows 0.
- **Narrative zero-state guard.** When every fantasy team has 0 playoff points AND 0 from the last scoring day, the Claude prompt now receives an explicit `ZERO-STATE` rule forbidding phrases like "came into today with N points" or "N-point gap" — with no games played yet there's nothing to reference.
- **`points_today` label re-worded.** Column heading on the League Live Board and the `StatCol` on Tonight are both now "Yesterday" instead of "Last" / "Last day" — clearer than a tooltip, matches how daily-fantasy contexts label the previous completed day.

### Added — Tonight game cards: NHL team + player links

Each player row in the Tonight section now leads with a compact 3-letter NHL team abbreviation so the home/away split inside a CAR-OTT card is unambiguous, and the player name itself is an anchor to `nhl.com/player/{id}` — matching the profile-link treatment that Series Rosters and MyStakes have had since v1.7.0.

### Fixed — goalies in the historical seed

The 5-year export included 18 goalie rows (Shesterkin, Bobrovsky, etc.) that slipped into `historical_playoff_skater_totals`. Fantasy format is skater-only, so these could never match a rostered player — dead weight that risked polluting the Bayesian prior's name-match. The Python parse script now filters `position == 'G'` at the seed boundary; the committed CSV shrinks from 600 to 582 rows.

### Tests

Lib suite at **53 passing** (was 11 before the rework). New coverage:
- 3 bracket-state correctness regressions.
- 3 carousel-to-BracketState classification tests.
- 6 playoff-Elo tests (seeding, upset payoff, blowout scaling, zero-sum, home-ice).
- 6 player-projection tests (cold start, heavy sample override, historical anchor, recency weighting, absent multiplier, empty input).
- 1 tie-splitting test (four empty rosters must each get 0.25 win-prob).
- 2 distribution tests (Gamma mean/variance, NegBin variance exceeds Poisson).
- 9 backtest helpers (Brier, log-loss, calibration, MAE/RMSE, interval coverage, bracket reconstruction).
- 1 home-ice test (owning home-ice raises advance-R1 probability when ratings are equal).

## v1.7.4 — 2026-04-18

### Changed
- **Pulse page reordered** — new `Tonight` section (merged "Today's Pulse" + "My Players In Action") moves to the top so the caller's first view is standing + games today, not the narrative. `Where You Stand` drops below it with a bigger yellow header matching the weight of the other section titles.
- **"Today" → "Last day" for points** — the `points_today` value is actually the last completed `daily_rankings` day (usually yesterday), not live scoring. Pulse StatCol now labels it `Last day`; League Live Board column renames `Today → Last`. The Claude narrative prompt (`pulse.rs`) gained an explicit rule: "points_today is the last completed scoring day, never 'today's points'" and the headline data line now reads `pts from the last completed scoring day` instead of `{} today`, so the columnist voice stops writing "pulling 3 today" on mornings where no games have happened.
- **Series Rosters: non-mine teams collapsible** — other fantasy teams' rosters now render as `<details>` collapsed by default with an Expand/Collapse pill; the caller's team stays pinned open with the yellow `YOU` border. The page is scannable in 14-team leagues again.

### Backend
- `backend/src/api/handlers/pulse.rs` narrative prompt reworded to prevent false "today's points" phrasing.

### Changed
- **Dashboard Overall Rankings shows all teams** — removed the 7-team cap; the home board now renders every fantasy team in the league. In a 10+ team league the old cap hid the bottom half of the standings behind a View All click.
- **Dropped redundant season badge on Dashboard** — the yellow `2025/2026 Playoffs` chip under `Overall Rankings` and `Sleepers` duplicated the `NHL 2026` label already shown in the NavBar. Removed the `dateBadge` prop on both tables.
- **Mobile menu gained Teams + Browse Leagues** — desktop has always exposed these under the user dropdown, but on mobile (`lg:hidden`) the user section jumped straight from the nav links to `League Settings`, leaving no way to reach the Teams page or switch leagues without going through the desktop breakpoint. Now mirrors the desktop dropdown.

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
