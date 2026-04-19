# Draft system

How a league goes from "we have users and a draft session" to "every user has a fantasy roster". Three subsystems:

1. HTTP endpoints that mutate `draft_sessions`, `player_pool`, `draft_picks`.
2. A WebSocket hub that broadcasts state changes to connected clients.
3. A player-pool builder that sources eligible players from either the stats leaderboard or the 16 playoff rosters.

## Lifecycle

```
 create              populate              start           pick → pick → pick …              finalize                 sleeper start → sleeper pick          complete
 POST                POST                  POST            POST /pick (N × rounds)            POST /finalize            POST /sleeper/start / /sleeper/pick   POST /complete
 /leagues/{id}/      /draft/{id}/          /draft/{id}/    each pick writes a row             copies draft_picks →      writes fantasy_sleepers              sets status='complete',
 draft               populate              start                                              fantasy_players           rows                                  completed_at
                                                                                              flips sleeper_status
 → draft_sessions    → player_pool         → status=       → draft_picks rows                 to 'active'                                                    
   row (pending)      rows                   'active'        session.current_pick_index ++                                                                   
                                             started_at
```

Status values on `draft_sessions.status`: `pending` → `active` → `picks_done` → `complete`. A separate `sleeper_status` field tracks the sleeper sub-round. A `paused` status pauses the regular draft without losing progress.

## HTTP endpoints

All draft endpoints require authentication. Full route listing in [`03-api.md`](./03-api.md). Relevant handlers are in [`backend/src/api/handlers/draft.rs`](../backend/src/api/handlers/draft.rs).

| Endpoint | Method | What it writes | Broadcasts |
| --- | --- | --- | --- |
| `/api/leagues/{id}/draft` | POST | `draft_sessions` + `player_pool` | - |
| `/api/leagues/{id}/draft/randomize-order` | POST | Shuffles `league_members.draft_order` | - |
| `/api/draft/{id}/populate` | POST | Rebuilds `player_pool` | `PlayerPoolUpdated` |
| `/api/draft/{id}/start` | POST | `status = 'active'`, `started_at` | `SessionUpdated` |
| `/api/draft/{id}/pause` | POST | `status = 'paused'` | `SessionUpdated` |
| `/api/draft/{id}/resume` | POST | `status = 'active'` | `SessionUpdated` |
| `/api/draft/{id}/pick` | POST | `draft_picks` row; advances `current_pick_index` | `PickMade` + `SessionUpdated` |
| `/api/draft/{id}/finalize` | POST | Copies picks to `fantasy_players`; sets `sleeper_status = 'active'` | `SessionUpdated` |
| `/api/draft/{id}/sleeper/start` | POST | `sleeper_status = 'active'` | `SessionUpdated` |
| `/api/draft/{id}/sleeper/pick` | POST | `fantasy_sleepers` row | `SleeperUpdated` |
| `/api/draft/{id}/complete` | POST | `status = 'complete'`, `completed_at` | `SessionUpdated` |

## Snake-draft math

Canonical snake: in round 1, member 1 picks first, member N picks last. In round 2, member N picks first and member 1 picks last. Pattern alternates across all rounds.

Implementation in [`handlers/draft.rs:309-321`](../backend/src/api/handlers/draft.rs):

```rust
// current_pick_index is a GLOBAL counter: 0, 1, 2, ..., total_rounds*num_members - 1
let pick_index = session.current_pick_index;
let round = pick_index / num_members;           // 0-based
let index_in_round = pick_index % num_members;

let member_index = if session.snake_draft && round % 2 == 1 {
    (num_members - 1) - index_in_round          // reverse on odd rounds
} else {
    index_in_round
};

let picking_member_id = &member_ids[member_index as usize];
```

Example with 4 members and 3 rounds (total 12 picks):

```
pick_index:  0  1  2  3  4  5  6  7  8  9 10 11
round:       0  0  0  0  1  1  1  1  2  2  2  2
in_round:    0  1  2  3  0  1  2  3  0  1  2  3
member:      0  1  2  3  3  2  1  0  0  1  2  3  ← order reverses on round 1
```

The pick row stores `pick_number = pick_index` (0-based global) and `round = round + 1` (1-based, for display). Finalize happens automatically when `current_pick_index >= total_rounds * num_members` - the handler flips `status` to `picks_done` and waits for an explicit `POST /finalize` from the admin.

## Player pool

File: [`backend/src/infra/jobs/player_pool.rs`](../backend/src/infra/jobs/player_pool.rs).

The pool is rebuilt by `POST /draft/{id}/populate`. Two branches depending on `game_type`:

### Regular season (game_type != 3)

`fetch_stats_leader_pool` ([`player_pool.rs:22-61`](../backend/src/infra/jobs/player_pool.rs)). Calls `NhlClient::get_skater_stats(season, game_type)`, walks nine stat categories from the response (goals, assists, points, PP goals, SH goals, plus/minus, faceoff leaders, penalty minutes, TOI), and deduplicates by `player_id` into a `HashMap<player_id, (name, position, team_abbrev, headshot_url)>`.

### Playoffs (game_type == 3)

`fetch_playoff_roster_pool_cached` ([`player_pool.rs:103-132`](../backend/src/infra/jobs/player_pool.rs)). Read-through cache:

1. Check `playoff_roster_cache` table for a row keyed by `(season, game_type)`. If present and deserialises cleanly, return it. One SELECT, no NHL calls.
2. On miss, call `fetch_playoff_roster_pool`: fetch the 16 rosters in parallel via `try_join_all`, merge into a `PoolMap`, write back to the cache table, and return.

The 10:00 UTC prewarm cron calls `refresh_playoff_roster_cache` explicitly, so in practice the cache is always warm during the playoffs.

The 16 team abbreviations come from `playoff_team_abbrevs` ([`player_pool.rs:161-178`](../backend/src/infra/jobs/player_pool.rs)): try the playoff carousel first; fall back to the top 16 teams by points percentage from `/v1/standings/now` if the carousel has fewer than 16 entries (which can happen briefly between regular-season end and when the NHL posts round 1 matchups).

## DraftHub - in-process broadcast

File: [`backend/src/ws/draft_hub.rs`](../backend/src/ws/draft_hub.rs).

`DraftHub` holds one `tokio::sync::broadcast::Sender<String>` per active draft session, inside an `RwLock<HashMap<String, Sender<String>>>`. Channel capacity is 64 messages ([`draft_hub.rs:54`](../backend/src/ws/draft_hub.rs)).

Subscribe logic uses double-checked locking to avoid write contention ([`draft_hub.rs:38-57`](../backend/src/ws/draft_hub.rs)): read-lock first, only acquire the write lock if the channel has to be created. Broadcast takes a read lock, serialises the event to JSON, and ignores send errors - there is no backpressure on the handlers.

### Event types

Defined at [`draft_hub.rs:9-23`](../backend/src/ws/draft_hub.rs):

```rust
#[derive(Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum DraftEvent {
    SessionUpdated {
        session_id: String,
        status: String,
        current_round: i32,
        current_pick_index: i32,
        sleeper_status: Option<String>,
        sleeper_pick_index: i32,
    },
    PickMade { pick: serde_json::Value },
    SleeperUpdated,
    PlayerPoolUpdated,
}
```

Serialised shape on the wire (camelCase):

```json
{ "type": "sessionUpdated", "data": { "sessionId": "...", "status": "active", ... } }
{ "type": "pickMade", "data": { "pick": { ... } } }
{ "type": "sleeperUpdated" }
{ "type": "playerPoolUpdated" }
```

### WebSocket handler

File: [`backend/src/ws/handler.rs`](../backend/src/ws/handler.rs).

Route: `GET /ws/draft/{session_id}` ([`routes.rs:276-279`](../backend/src/api/routes.rs)).

`handle_draft_ws` runs one `tokio::select!` loop with three arms ([`ws/handler.rs:36-89`](../backend/src/ws/handler.rs)):

1. **Ping** - every `WS_PING_INTERVAL` (30 s, [`tuning.rs:241`](../backend/src/tuning.rs)) send a WebSocket Ping to keep the connection alive through proxies.
2. **Broadcast → client** - messages arriving on the `broadcast::Receiver<String>` are forwarded as `Message::Text`. On `Lagged(n)`, the client has missed `n` messages (buffer size 64); log and continue. On `Closed`, break the loop.
3. **Client → server** - Ping/Pong are echoed; Close ends the loop; Text and Binary messages are silently ignored. The protocol is server-push only.

## Frontend reconnect

File: `frontend/src/lib/realtime.ts`.

The React client derives the WebSocket URL by taking the API URL and rewriting `https → wss` / `http → ws`, then appending `?token=<auth_token>`. On `onopen` the backoff timer resets; on `onclose` an exponential-backoff reconnect fires with a 30-second cap.

Incoming messages are dispatched to handlers by their `type` field:

- `sessionUpdated` → updates React Query cache for the draft session
- `pickMade` → appends to the picks query cache
- `sleeperUpdated` → refetches sleepers
- `playerPoolUpdated` → refetches the player pool

Details in [`10-frontend-data-flow.md`](./10-frontend-data-flow.md).

## Sequence diagram - one pick

```
user (React)              backend                       Postgres
    │                         │                             │
    │  POST /api/draft/      │                             │
    │  {id}/pick              │                             │
    ├────────────────────────►│                             │
    │                         │  SELECT draft_sessions      │
    │                         ├────────────────────────────►│
    │                         │                             │
    │                         │  SELECT player_pool         │
    │                         ├────────────────────────────►│
    │                         │                             │
    │                         │  INSERT draft_picks         │
    │                         ├────────────────────────────►│
    │                         │                             │
    │                         │  UPDATE draft_sessions      │
    │                         │    current_pick_index++     │
    │                         ├────────────────────────────►│
    │                         │                             │
    │                         │  DraftHub.broadcast(        │
    │                         │     PickMade)               │
    │                         │  DraftHub.broadcast(        │
    │                         │     SessionUpdated)         │
    │                         │     (in-process)            │
    │                         │                             │
    │  200 OK (pick JSON)    │                             │
    │◄────────────────────────┤                             │
    │                         │                             │

other users (all connected via /ws/draft/{id}):
    │                         │  Message::Text(PickMade)   │
    │◄────────────────────────┤                             │
    │                         │  Message::Text(Session)    │
    │◄────────────────────────┤                             │
    │  React Query cache      │                             │
    │  update, UI repaints    │                             │
```

## Draft configuration

Values live on `draft_sessions`:

| Column | Default | Meaning |
| --- | --- | --- |
| `total_rounds` | 10 | How many rounds of picks |
| `snake_draft` | true | Snake vs linear |
| `current_round` | 1 | 1-based, advanced on each pick |
| `current_pick_index` | 0 | 0-based global pick counter |
| `sleeper_status` | null | `null` during main draft; `'active'` / `'complete'` during sleeper round |
| `sleeper_pick_index` | 0 | 0-based counter for the sleeper round |

The draft randomize-order endpoint only writes `league_members.draft_order`. The pick math reads the ordered member IDs from `get_league_member_ids_ordered(league_id)`.

## What the draft system does not do

- No pick timer enforced by the server. If a draft drags, it drags. The frontend surfaces elapsed time cosmetically via `DRAFT_ELAPSED_TICK_MS`, but nothing auto-picks.
- No trade-up or skip-turn flows.
- No ADP sorting or recommendation engine. The pool is flat and searchable.
- No draft history export. Picks remain in `draft_picks` after finalize for display but are never replayed.
