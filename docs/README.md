# Fantasy Puck - technical documentation

Fantasy Puck is a skater-only fantasy hockey app for small leagues. Users create a league, draft NHL skaters, and score by goals and assists through the playoffs. The backend is a Rust/Axum service that mirrors the NHL web API into Postgres, runs a Monte Carlo simulator to project the fantasy-points race, and serves a React/TypeScript frontend.

These documents describe how the system works today. They are reference material, not tutorials - each one assumes familiarity with the language and framework, and links back to specific source files so a curious reader can jump straight to code.

## Documents

| # | Document | Purpose |
| --- | --- | --- |
| 01 | [Backend architecture](./01-architecture.md) | Module tree, layering, request flow, boot sequence, middleware stack |
| 02 | [Database schema](./02-database.md) | Every table and view, grouped by fantasy state / draft / NHL mirror / cache |
| 03 | [HTTP API](./03-api.md) | Route listing with method, path, handler, auth, data source, cache keys |
| 04 | [NHL integration](./04-nhl-integration.md) | NHL HTTP client, mirror tables, meta / live / edge pollers |
| 05 | [Prediction engine](./05-prediction-engine.md) | Team ratings, Elo, goalie bonus, player projection, Monte Carlo, calibration |
| 06 | [Business logic](./06-business-logic.md) | Fantasy scoring rule, live data flow, daily rankings snapshot |
| 07 | [Background jobs](./07-background-jobs.md) | Scheduled crons, continuous pollers, startup one-shots, admin triggers |
| 08 | [Draft system](./08-draft.md) | Draft lifecycle, snake math, WebSocket hub, sleeper round |
| 09 | [Frontend architecture](./09-frontend-architecture.md) | Routing, contexts, feature folders, design system |
| 10 | [Frontend data flow](./10-frontend-data-flow.md) | React Query keys, staleTimes, per-page polling, WebSocket wiring |

## Where to start

### New backend contributor

1. [01 - Architecture](./01-architecture.md) for the module layout.
2. [02 - Database](./02-database.md) to understand the data model.
3. [04 - NHL integration](./04-nhl-integration.md) and [07 - Background jobs](./07-background-jobs.md) for how data gets into the database.
4. [06 - Business logic](./06-business-logic.md) for how fantasy points are computed.
5. [05 - Prediction engine](./05-prediction-engine.md) when you need to understand the forward model.

### New frontend contributor

1. [09 - Frontend architecture](./09-frontend-architecture.md) for the layout.
2. [10 - Frontend data flow](./10-frontend-data-flow.md) for which page calls what.
3. [03 - HTTP API](./03-api.md) when you need endpoint shapes.
4. [08 - Draft](./08-draft.md) if you are touching the draft WebSocket.

### Operator debugging a production issue

1. [07 - Background jobs](./07-background-jobs.md) to find which cron or poller owns the pipeline that is misbehaving.
2. [04 - NHL integration](./04-nhl-integration.md) for rate-limit, retry, and mirror-table behaviour.
3. [06 - Business logic](./06-business-logic.md) for the timing of live-to-snapshot transitions.
4. [03 - HTTP API](./03-api.md) for admin-endpoint triggers (`/api/admin/prewarm`, `/api/admin/rehydrate`, `/api/admin/process-rankings/{date}`, `/api/admin/cache/invalidate`).

## Related

- [`CLAUDE.md`](../CLAUDE.md) at the repo root - high-level commands, code style conventions, and the season / game-type switch procedure.
- [`AGENTS.md`](../AGENTS.md) at the repo root - agent-facing quickstart.
- [`CHANGELOG.md`](../CHANGELOG.md) - release history.
