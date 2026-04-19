-- NHL Edge telemetry mirror.
--
-- Populated by the nightly `edge_refresher` job (see
-- `backend/src/infra/jobs/edge_refresher.rs`). Insights reads from
-- here instead of fanning out to NHL at request time — the previous
-- design issued one `/player/{id}/landing` call per hot player per
-- cache miss, which added five bonus NHL calls to every cold render
-- and contributed to the playoff-evening 429 cascade.
--
-- The refresher covers the top ~30 season-leaderboard skaters once
-- per day. Players not in that cohort have no row and render with
-- blank speed tiles, which is the correct UX degradation — better
-- than blocking the page on a live fetch.

CREATE TABLE IF NOT EXISTS public.nhl_skater_edge (
    player_id           BIGINT PRIMARY KEY,
    top_speed_mph       REAL,
    top_shot_speed_mph  REAL,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
