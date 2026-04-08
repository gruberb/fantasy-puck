# Fantasy Puck API Reference

Base URL: `/api` (HTTP) and `/ws` (WebSocket)

---

## Response Format

All HTTP endpoints return a consistent JSON envelope.

**Success response:**

```json
{
  "success": true,
  "data": <T>
}
```

**Error response:**

```json
{
  "success": false,
  "error": "Human-readable error message"
}
```

### HTTP Status Codes

| Code | Meaning | Error Variant |
|------|---------|---------------|
| 200 | OK | -- |
| 400 | Bad Request | `Validation` |
| 401 | Unauthorized | `Unauthorized` |
| 403 | Forbidden | `Forbidden` |
| 404 | Not Found | `NotFound` |
| 500 | Internal Server Error | `Database`, `Internal` |
| 502 | Bad Gateway | `NhlApi` |

---

## Authentication

All endpoints marked **Auth: yes** require a `Bearer` token in the `Authorization` header:

```
Authorization: Bearer <jwt>
```

Tokens are issued by `POST /api/auth/login` and `POST /api/auth/register`.

---

## Query Parameter Conventions

| Parameter | Used By | Format | Notes |
|-----------|---------|--------|-------|
| `league_id` | Most fantasy/NHL endpoints | UUID string | Scopes data to a league |
| `date` | Games, daily rankings | `YYYY-MM-DD` | Required where noted |
| `season` | NHL endpoints | `YYYYYYYY` (e.g. `20252026`) | Eight-digit combined season |
| `game_type` | NHL skaters | Integer (`2` = regular, `3` = playoffs) | |
| `visibility` | League list | `"public"` or omit for all | |
| `detail` | Games list | `"extended"` for full boxscore overlay | |
| `scope` | Cache invalidation | `"all"`, `"today"`, or `YYYY-MM-DD` | |

---

## Auth Routes

### POST /api/auth/register

Create a new user account.

| | |
|---|---|
| **Auth** | No |
| **Body** | `{ "email": string, "password": string, "displayName": string }` |
| **Response** | `AuthResponse` |

```json
{
  "success": true,
  "data": {
    "token": "jwt...",
    "user": { "id": "uuid", "email": "user@example.com" },
    "profile": { "displayName": "Name", "isAdmin": false }
  }
}
```

**Errors:** 400 if email already registered.

---

### POST /api/auth/login

Authenticate an existing user.

| | |
|---|---|
| **Auth** | No |
| **Body** | `{ "email": string, "password": string }` |
| **Response** | `AuthResponse` (same shape as register) |

**Errors:** 401 if email/password invalid.

---

### GET /api/auth/me

Return the currently authenticated user.

| | |
|---|---|
| **Auth** | Yes |
| **Params** | None |
| **Response** | `MeResponse` |

```json
{
  "success": true,
  "data": {
    "user": { "id": "uuid", "email": "user@example.com" },
    "profile": { "displayName": "Name", "isAdmin": false }
  }
}
```

---

### PUT /api/auth/profile

Update the authenticated user's display name.

| | |
|---|---|
| **Auth** | Yes |
| **Body** | `{ "displayName": string }` |
| **Response** | `null` (success envelope only) |

---

### DELETE /api/auth/account

Permanently delete the authenticated user's account and all associated data.

| | |
|---|---|
| **Auth** | Yes |
| **Params** | None |
| **Response** | `null` |

---

### GET /api/auth/memberships

List all league memberships for the authenticated user.

| | |
|---|---|
| **Auth** | Yes |
| **Params** | None |
| **Response** | `MembershipRow[]` |

```json
{
  "success": true,
  "data": [
    {
      "leagueId": "uuid",
      "leagueName": "My League",
      "leagueSeason": "20252026",
      "fantasyTeamId": 42,
      "teamName": "Team Name",
      "draftOrder": 1
    }
  ]
}
```

---

## League Routes

### GET /api/leagues

List leagues.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?visibility=public` (optional; omit for all leagues) |
| **Response** | `League[]` |

```json
{
  "success": true,
  "data": [
    { "id": "uuid", "name": "League Name", "season": "20252026", "visibility": "public", "createdBy": "uuid" }
  ]
}
```

---

### POST /api/leagues

Create a new league. The authenticated user becomes the first member.

| | |
|---|---|
| **Auth** | Yes |
| **Body** | `{ "name": string, "season"?: string }` |
| **Response** | `LeagueRow` |

`season` defaults to `"20252026"` if omitted.

```json
{
  "success": true,
  "data": {
    "id": "uuid",
    "name": "My League",
    "season": "20252026",
    "visibility": "private",
    "created_by": "uuid"
  }
}
```

---

### DELETE /api/leagues/{league_id}

Delete a league and all associated data.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `league_id` -- UUID |
| **Response** | `null` |

---

### GET /api/leagues/{league_id}/members

List all members of a league.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `league_id` -- UUID |
| **Response** | `LeagueMemberRow[]` |

```json
{
  "success": true,
  "data": [
    {
      "id": "uuid",
      "userId": "uuid",
      "draftOrder": 1,
      "displayName": "Player Name",
      "teamName": "My Team",
      "fantasyTeamId": 42
    }
  ]
}
```

---

### POST /api/leagues/{league_id}/join

Join an existing league.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `league_id` -- UUID |
| **Body** | `{ "teamName": string }` |
| **Response** | `null` |

---

### DELETE /api/leagues/{league_id}/members/{member_id}

Remove a member from a league.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `league_id` -- UUID, `member_id` -- UUID |
| **Response** | `null` |

Validates that the member belongs to the specified league before removal.

---

## Draft Routes

### GET /api/leagues/{league_id}/draft

Get the draft session for a league (if one exists), including all picks and the player pool.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `league_id` -- UUID |
| **Response** | `DraftStateResponse | null` |

```json
{
  "success": true,
  "data": {
    "session": {
      "id": "uuid",
      "leagueId": "uuid",
      "status": "pending|active|paused|picks_done|completed",
      "currentRound": 1,
      "currentPickIndex": 0,
      "totalRounds": 5,
      "snakeDraft": true,
      "startedAt": "2025-04-01T12:00:00Z",
      "completedAt": null,
      "sleeperStatus": null,
      "sleeperPickIndex": 0
    },
    "picks": [ "...DraftPickRow[]" ],
    "playerPool": [ "...PlayerPoolRow[]" ]
  }
}
```

Returns `null` in `data` if no draft session exists for the league.

---

### POST /api/leagues/{league_id}/draft

Create a new draft session. Automatically populates the player pool from NHL regular-season stats.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `league_id` -- UUID |
| **Body** | `{ "totalRounds": number, "snakeDraft": boolean }` |
| **Response** | `DraftSessionRow` |

---

### POST /api/leagues/{league_id}/draft/randomize-order

Randomize the draft order for all members in the league.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `league_id` -- UUID |
| **Response** | `null` |

---

### GET /api/draft/{draft_id}

Get the full state of a draft: session metadata, all picks, and the player pool.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `DraftStateResponse` |

Same shape as `GET /api/leagues/{league_id}/draft` but always returns a value (404 if not found).

---

### DELETE /api/draft/{draft_id}

Delete a draft session and all associated picks/pool data.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `null` |

---

### POST /api/draft/{draft_id}/populate

Re-populate the player pool from the NHL API. Clears any existing pool data and fetches fresh stats.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `PlayerPoolRow[]` |

```json
{
  "success": true,
  "data": [
    {
      "id": "uuid",
      "draftSessionId": "uuid",
      "nhlId": 8478402,
      "name": "Connor McDavid",
      "position": "C",
      "nhlTeam": "EDM",
      "headshotUrl": "https://assets.nhle.com/mugs/nhl/latest/8478402.png"
    }
  ]
}
```

---

### POST /api/draft/{draft_id}/start

Transition a draft from `pending` to `active`. Broadcasts a `sessionUpdated` WebSocket event.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `DraftSessionRow` |

---

### POST /api/draft/{draft_id}/pause

Pause an active draft. Broadcasts `sessionUpdated`.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `DraftSessionRow` |

---

### POST /api/draft/{draft_id}/resume

Resume a paused draft. Broadcasts `sessionUpdated`.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `DraftSessionRow` |

---

### POST /api/draft/{draft_id}/pick

Make a draft pick. The server determines which member is picking based on `currentPickIndex`, round, and snake-draft rules. Broadcasts both `pickMade` and `sessionUpdated` WebSocket events.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Body** | `{ "playerPoolId": string }` |
| **Response** | `DraftPickRow` |

```json
{
  "success": true,
  "data": {
    "id": "uuid",
    "draftSessionId": "uuid",
    "leagueMemberId": "uuid",
    "playerPoolId": "uuid",
    "nhlId": 8478402,
    "playerName": "Connor McDavid",
    "nhlTeam": "EDM",
    "position": "C",
    "round": 1,
    "pickNumber": 0,
    "pickedAt": "2025-04-01T12:05:00Z"
  }
}
```

**Errors:**
- 400 `"Draft is not active"` -- draft must have status `active`.
- 400 `"All rounds are complete"` -- all picks already made.
- 400 `"Player already drafted"` -- player was picked by another team.
- 400 `"No members in league"` -- league has no members.

When all picks are complete the session status transitions to `picks_done`.

---

### POST /api/draft/{draft_id}/finalize

Finalize the main draft: syncs all draft picks into the `fantasy_players` table and begins the sleeper round (sets `sleeperStatus` to `active`). Broadcasts `sessionUpdated`.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `null` |

---

### POST /api/draft/{draft_id}/complete

Mark the entire draft (including sleeper round) as `completed`. Broadcasts `sessionUpdated`.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `null` |

---

### GET /api/draft/{draft_id}/sleepers

Get all undrafted players eligible for the sleeper round.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `PlayerPoolRow[]` |

---

### GET /api/draft/{draft_id}/sleeper-picks

Get all sleeper picks made so far in this draft's league.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `SleeperPick[]` |

```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "teamId": 42,
      "nhlId": 8478402,
      "name": "Player Name",
      "position": "C",
      "nhlTeam": "EDM"
    }
  ]
}
```

---

### POST /api/draft/{draft_id}/sleeper/start

Start the sleeper round. Broadcasts `sleeperUpdated`.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Response** | `DraftSessionRow` |

---

### POST /api/draft/{draft_id}/sleeper/pick

Make a sleeper pick. Each team gets exactly one sleeper pick. Broadcasts both `sessionUpdated` and `sleeperUpdated`.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `draft_id` -- UUID |
| **Body** | `{ "playerPoolId": string, "teamId": number }` |
| **Response** | `null` |

**Errors:**
- 400 `"Sleeper round is not active"` -- `sleeperStatus` must be `active`.
- 400 `"All sleeper picks are done"` -- every team already picked.
- 400 `"This team already has a sleeper pick"` -- duplicate pick attempt.
- 400 `"This player is already picked as a sleeper by another team"` -- player taken.

When all sleeper picks are complete, `sleeperStatus` transitions to `completed`.

---

## Fantasy Team Routes

### GET /api/fantasy/teams

List all fantasy teams in a league.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Response** | `FantasyTeam[]` |

---

### GET /api/fantasy/teams/{id}

Get a single fantasy team with calculated player points.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Path** | `id` -- integer team ID |
| **Response** | `TeamPointsResponse` |

```json
{
  "success": true,
  "data": {
    "teamId": 42,
    "teamName": "My Team",
    "players": [
      {
        "name": "Connor McDavid",
        "nhlTeam": "EDM",
        "nhlId": 8478402,
        "position": "C",
        "goals": 15,
        "assists": 30,
        "totalPoints": 45,
        "imageUrl": "https://...",
        "teamLogo": "https://..."
      }
    ],
    "teamTotals": {
      "goals": 50,
      "assists": 80,
      "totalPoints": 130
    }
  }
}
```

---

### PUT /api/fantasy/teams/{id}

Rename a fantasy team.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `id` -- integer team ID |
| **Body** | `{ "name": string }` |
| **Response** | `null` |

---

### POST /api/fantasy/teams/{id}/players

Add a player to a fantasy team (manual roster edit outside draft).

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `id` -- integer team ID |
| **Body** | `{ "nhlId": number, "name": string, "position": string, "nhlTeam": string }` |
| **Response** | `FantasyPlayer` |

---

### DELETE /api/fantasy/players/{player_id}

Remove a player from their fantasy team.

| | |
|---|---|
| **Auth** | Yes |
| **Path** | `player_id` -- integer |
| **Response** | `null` |

---

### GET /api/fantasy/rankings

Get current total rankings for all fantasy teams in a league, calculated from live NHL stats.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Response** | `TeamRanking[]` |

Each entry contains `teamId`, `teamName`, `totalPoints`, `goals`, `assists`, and `rank`.

---

### GET /api/fantasy/rankings/daily

Get daily rankings based on game-day boxscores.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>&date=YYYY-MM-DD` (both required) |
| **Response** | `DailyRankingsResponse` |

```json
{
  "success": true,
  "data": {
    "date": "2025-04-08",
    "rankings": [
      {
        "teamId": 42,
        "teamName": "My Team",
        "totalPoints": 12,
        "totalGoals": 5,
        "totalAssists": 7,
        "playerPerformances": [ "..." ]
      }
    ]
  }
}
```

**Errors:** 404 if no games found for the date.

---

### GET /api/fantasy/rankings/playoffs

Compute playoff-adjusted rankings combining base points, team bets, NHL playoff survival, and top-10 skater ownership.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Response** | `PlayoffRankingResponse[]` |

Each entry includes `rank`, `teamId`, `teamName`, `totalPoints`, `playoffScore`, `teamsAlive`, `topPlayerCount`, and breakdown details.

---

### GET /api/fantasy/team-bets

Get a breakdown of which NHL teams each fantasy team has "bet on" (i.e., how many players they hold from each NHL team).

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Response** | `FantasyTeamBetsResponse[]` |

```json
{
  "success": true,
  "data": [
    {
      "teamId": 42,
      "teamName": "My Team",
      "bets": [
        { "nhlTeam": "EDM", "nhlTeamName": "Edmonton Oilers", "numPlayers": 3, "teamLogo": "https://..." }
      ]
    }
  ]
}
```

---

### GET /api/fantasy/players

Get all fantasy players grouped by NHL team, with ownership info.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Response** | `NhlTeamPlayersResponse[]` |

```json
{
  "success": true,
  "data": [
    {
      "nhlTeam": "EDM",
      "teamLogo": "https://...",
      "players": [
        {
          "nhlId": 8478402,
          "name": "Connor McDavid",
          "fantasyTeamId": 42,
          "fantasyTeamName": "My Team",
          "position": "C",
          "nhlTeam": "EDM",
          "imageUrl": "https://..."
        }
      ]
    }
  ]
}
```

---

### GET /api/fantasy/team-stats

Get detailed statistics for all fantasy teams in a league, including daily ranking history, top players, and top NHL team contributions.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Response** | `TeamStatsResponse[]` |

```json
{
  "success": true,
  "data": [
    {
      "teamId": 42,
      "teamName": "My Team",
      "totalPoints": 130,
      "dailyWins": 5,
      "dailyTopThree": 12,
      "winDates": ["2025-04-01", "2025-04-03"],
      "topThreeDates": ["2025-04-01", "2025-04-02"],
      "topPlayers": [
        { "nhlId": 8478402, "name": "Connor McDavid", "points": 45, "nhlTeam": "EDM", "position": "C", "imageUrl": "...", "teamLogo": "..." }
      ],
      "topNhlTeams": [
        { "nhlTeam": "EDM", "points": 80, "teamLogo": "...", "teamName": "Edmonton Oilers" }
      ]
    }
  ]
}
```

---

### GET /api/fantasy/sleepers

Get all sleeper picks in a league with their current NHL stats.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Response** | `SleeperStatsResponse[]` |

```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "nhlId": 8478402,
      "name": "Player Name",
      "nhlTeam": "EDM",
      "position": "C",
      "fantasyTeam": "My Team",
      "fantasyTeamId": 42,
      "goals": 5,
      "assists": 8,
      "totalPoints": 13,
      "plusMinus": 3,
      "timeOnIce": "18.5",
      "imageUrl": "https://...",
      "teamLogo": "https://..."
    }
  ]
}
```

Results are sorted by `totalPoints` descending.

---

## NHL Data Routes

### GET /api/nhl/skaters/top

Get top skaters consolidated across all stat categories.

| | |
|---|---|
| **Auth** | No |
| **Query** | See below |
| **Response** | `ConsolidatedPlayerStats[]` |

**Required query parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `limit` | integer | Max players to return |
| `season` | integer | e.g. `20252026` |
| `game_type` | integer | `2` = regular, `3` = playoffs |
| `form_games` | integer | Number of recent games for form calculation |

**Optional query parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `include_form` | boolean | `false` | Include recent-form stats per player |
| `league_id` | string | -- | Include fantasy team ownership overlay |

```json
{
  "success": true,
  "data": [
    {
      "id": 8478402,
      "firstName": "Connor",
      "lastName": "McDavid",
      "sweaterNumber": 97,
      "headshot": "https://...",
      "teamAbbrev": "EDM",
      "teamName": "EDM",
      "teamLogo": "https://...",
      "position": "C",
      "stats": {
        "goals": 25, "assists": 45, "points": 70,
        "goalsPp": 8, "goalsSh": 1, "plusMinus": 15,
        "faceoffPct": 52, "penaltyMins": 12, "toi": 1200
      },
      "fantasyTeam": { "teamId": 42, "teamName": "My Team" },
      "form": { "games": 5, "goals": 3, "assists": 5, "points": 8 }
    }
  ]
}
```

`fantasyTeam` is `null` if the player is unowned or `league_id` is omitted. `form` is `null` if `include_form` is false.

---

### GET /api/nhl/games

Get NHL games for a specific date, with optional fantasy overlay.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?date=YYYY-MM-DD` (required), `?league_id=<uuid>` (optional), `?detail=extended` (optional) |
| **Response** | `TodaysGamesResponse` |

```json
{
  "success": true,
  "data": {
    "date": "2025-04-08",
    "games": [
      {
        "id": 2024020001,
        "homeTeam": "EDM",
        "awayTeam": "CGY",
        "startTime": "2025-04-08T01:00:00Z",
        "venue": "Rogers Place",
        "homeTeamPlayers": [ "...FantasyPlayerInGame" ],
        "awayTeamPlayers": [ "...FantasyPlayerInGame" ],
        "homeTeamLogo": "https://...",
        "awayTeamLogo": "https://...",
        "homeScore": 3,
        "awayScore": 1,
        "gameState": "FINAL",
        "period": "3 Period",
        "seriesStatus": null
      }
    ],
    "summary": {
      "totalGames": 5,
      "totalTeamsPlaying": 10,
      "teamPlayersCount": [
        { "nhlTeam": "EDM", "playerCount": 4 }
      ]
    },
    "fantasyTeams": null
  }
}
```

When `detail=extended` and `league_id` are both provided, `fantasyTeams` is populated with per-team extended player data (including form, TOI, and playoff stats). Responses are cached and auto-refreshed when games go live.

---

### GET /api/nhl/match-day

Get today's match-day view: all games, fantasy team breakdowns, and player performance. Automatically detects "today" in NHL Eastern time and includes late-running games from the previous night.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Response** | `MatchDayResponse` |

```json
{
  "success": true,
  "data": {
    "date": "2025-04-08",
    "games": [ "...MatchDayGameResponse[]" ],
    "fantasyTeams": [
      {
        "teamId": 42,
        "teamName": "My Team",
        "playersInAction": [ "...extended player data" ],
        "totalPlayersToday": 5
      }
    ],
    "summary": { "totalGames": 5, "totalTeamsPlaying": 10, "teamPlayersCount": [] }
  }
}
```

Responses are cached. Live games trigger real-time score/stat updates on each request.

---

### GET /api/nhl/playoffs

Get the NHL playoff bracket/carousel data.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?season=YYYYYYYY` (required, e.g. `20252026`) |
| **Response** | `PlayoffCarouselResponse` |

Includes series matchups, game results, and computed state (teams eliminated vs. still alive).

**Errors:**
- 400 if season format is invalid.
- 404 if playoff data is not available for the season.

---

### GET /api/nhl/roster/{team}

Get the full NHL roster for a team.

| | |
|---|---|
| **Auth** | No |
| **Path** | `team` -- 3-letter team abbreviation (e.g. `EDM`, case-insensitive) |
| **Response** | `NhlRosterPlayer[]` |

```json
{
  "success": true,
  "data": [
    {
      "nhlId": 8478402,
      "name": "Connor McDavid",
      "position": "C",
      "team": "EDM",
      "headshotUrl": "https://assets.nhle.com/mugs/nhl/latest/8478402.png"
    }
  ]
}
```

---

## Insights Route

### GET /api/insights

Generate AI-powered insights for a league's fantasy matchup. Combines multiple data signals (hot players, cup contenders, today's games, fantasy race, sleeper alerts, news headlines) and generates LLM narratives. Responses are cached per league per day.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?league_id=<uuid>` (required) |
| **Response** | `InsightsResponse` |

```json
{
  "success": true,
  "data": {
    "generatedAt": "2025-04-08T14:30:00Z",
    "narratives": {
      "todaysWatch": "Narrative about today's key matchups...",
      "gameNarratives": ["Per-game narrative..."],
      "hotPlayers": "Who's on fire right now...",
      "cupContenders": "Playoff picture analysis...",
      "fantasyRace": "League standings narrative...",
      "sleeperWatch": "Sleeper pick performance update..."
    },
    "signals": {
      "hotPlayers": [ { "nhlId": 8478402, "name": "...", "nhlTeam": "EDM", "fantasyTeam": "My Team", "points": 8, "goals": 3, "assists": 5 } ],
      "cupContenders": [],
      "todaysGames": [],
      "fantasyRace": [],
      "sleeperAlerts": [],
      "newsHeadlines": []
    }
  }
}
```

---

## Admin Routes

### GET /api/admin/process-rankings/{date}

Trigger daily ranking processing for all leagues on a given date. Typically called by a cron job.

| | |
|---|---|
| **Auth** | No |
| **Path** | `date` -- `YYYY-MM-DD` |
| **Response** | `string` |

```json
{
  "success": true,
  "data": "Rankings processed for 2025-04-08 across 3 leagues"
}
```

---

### GET /api/admin/cache/invalidate

Invalidate cached data.

| | |
|---|---|
| **Auth** | No |
| **Query** | `?scope=<value>` (optional) |
| **Response** | `string` |

**Scope values:**

| Value | Effect |
|-------|--------|
| `all` | Invalidates all DB cache entries and NHL API in-memory cache |
| `today` | Invalidates cache entries for today's date (Eastern time) |
| `YYYY-MM-DD` | Invalidates cache entries for a specific date |
| _(omitted)_ | Invalidates only the match-day cache for today |

---

## WebSocket

### WS /ws/draft/{session_id}

Real-time draft updates. Connect to receive server-pushed events for a specific draft session.

**Connection:** `ws://host/ws/draft/<session_id>`

The server sends JSON messages. The client should only send ping frames (pong is handled automatically). All other client messages are ignored.

### Event Types

Events are serialized as tagged JSON with `type` and `data` fields:

#### sessionUpdated

Broadcast whenever the draft session state changes (start, pause, resume, pick, finalize, complete).

```json
{
  "type": "sessionUpdated",
  "data": {
    "sessionId": "uuid",
    "status": "active",
    "currentRound": 2,
    "currentPickIndex": 5,
    "sleeperStatus": null,
    "sleeperPickIndex": 0
  }
}
```

**`status` values:** `pending`, `active`, `paused`, `picks_done`, `completed`

**`sleeperStatus` values:** `null`, `active`, `completed`

#### pickMade

Broadcast when a player is drafted.

```json
{
  "type": "pickMade",
  "data": {
    "pick": {
      "id": "uuid",
      "draftSessionId": "uuid",
      "leagueMemberId": "uuid",
      "playerPoolId": "uuid",
      "nhlId": 8478402,
      "playerName": "Connor McDavid",
      "nhlTeam": "EDM",
      "position": "C",
      "round": 1,
      "pickNumber": 0,
      "pickedAt": "2025-04-01T12:05:00Z"
    }
  }
}
```

#### sleeperUpdated

Broadcast when a sleeper pick is made or the sleeper round starts. This event has no `data` payload -- clients should re-fetch sleeper picks via the REST API.

```json
{
  "type": "sleeperUpdated"
}
```
