-- Users table (replaces Supabase auth.users)
CREATE TABLE IF NOT EXISTS public.users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Profiles table
CREATE TABLE IF NOT EXISTS public.profiles (
    id UUID PRIMARY KEY REFERENCES public.users(id) ON DELETE CASCADE,
    display_name TEXT NOT NULL DEFAULT '',
    is_admin BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Leagues
CREATE TABLE IF NOT EXISTS public.leagues (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    season TEXT NOT NULL DEFAULT '20252026',
    visibility TEXT NOT NULL DEFAULT 'public',
    created_by UUID REFERENCES public.users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Fantasy teams
CREATE TABLE IF NOT EXISTS public.fantasy_teams (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    user_id UUID REFERENCES public.users(id) ON DELETE CASCADE,
    league_id UUID REFERENCES public.leagues(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- League members
CREATE TABLE IF NOT EXISTS public.league_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    league_id UUID NOT NULL REFERENCES public.leagues(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES public.users(id) ON DELETE CASCADE,
    fantasy_team_id BIGINT REFERENCES public.fantasy_teams(id) ON DELETE SET NULL,
    draft_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(league_id, user_id)
);

-- Fantasy players (drafted roster)
CREATE TABLE IF NOT EXISTS public.fantasy_players (
    id BIGSERIAL PRIMARY KEY,
    team_id BIGINT NOT NULL REFERENCES public.fantasy_teams(id) ON DELETE CASCADE,
    nhl_id BIGINT NOT NULL,
    name TEXT NOT NULL,
    position TEXT NOT NULL DEFAULT '',
    nhl_team TEXT NOT NULL DEFAULT ''
);

-- Fantasy sleepers
CREATE TABLE IF NOT EXISTS public.fantasy_sleepers (
    id BIGSERIAL PRIMARY KEY,
    team_id BIGINT REFERENCES public.fantasy_teams(id) ON DELETE CASCADE,
    nhl_id BIGINT NOT NULL,
    name TEXT NOT NULL,
    position TEXT NOT NULL DEFAULT '',
    nhl_team TEXT NOT NULL DEFAULT ''
);

-- Draft sessions
CREATE TABLE IF NOT EXISTS public.draft_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    league_id UUID NOT NULL REFERENCES public.leagues(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending',
    current_round INTEGER NOT NULL DEFAULT 1,
    current_pick_index INTEGER NOT NULL DEFAULT 0,
    total_rounds INTEGER NOT NULL DEFAULT 10,
    snake_draft BOOLEAN NOT NULL DEFAULT true,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    sleeper_status TEXT,
    sleeper_pick_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Player pool (available players for a draft)
CREATE TABLE IF NOT EXISTS public.player_pool (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    draft_session_id UUID NOT NULL REFERENCES public.draft_sessions(id) ON DELETE CASCADE,
    nhl_id BIGINT NOT NULL,
    name TEXT NOT NULL,
    position TEXT NOT NULL DEFAULT '',
    nhl_team TEXT NOT NULL DEFAULT '',
    headshot_url TEXT NOT NULL DEFAULT ''
);

-- Draft picks
CREATE TABLE IF NOT EXISTS public.draft_picks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    draft_session_id UUID NOT NULL REFERENCES public.draft_sessions(id) ON DELETE CASCADE,
    league_member_id UUID NOT NULL REFERENCES public.league_members(id) ON DELETE CASCADE,
    player_pool_id UUID REFERENCES public.player_pool(id) ON DELETE SET NULL,
    nhl_id BIGINT NOT NULL,
    player_name TEXT NOT NULL,
    nhl_team TEXT NOT NULL DEFAULT '',
    position TEXT NOT NULL DEFAULT '',
    round INTEGER NOT NULL,
    pick_number INTEGER NOT NULL,
    picked_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Daily rankings (for historical tracking)
CREATE TABLE IF NOT EXISTS public.daily_rankings (
    id BIGSERIAL PRIMARY KEY,
    team_id BIGINT NOT NULL REFERENCES public.fantasy_teams(id) ON DELETE CASCADE,
    league_id UUID NOT NULL REFERENCES public.leagues(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    rank INTEGER NOT NULL,
    points INTEGER NOT NULL DEFAULT 0,
    goals INTEGER NOT NULL DEFAULT 0,
    assists INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(team_id, date)
);

-- Response cache
CREATE TABLE IF NOT EXISTS public.response_cache (
    cache_key TEXT PRIMARY KEY,
    data TEXT NOT NULL,
    date TEXT,
    created_at TEXT,
    last_updated TEXT
);

-- Delete user account function
CREATE OR REPLACE FUNCTION public.delete_user_account(target_user_id UUID)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    -- Cascading deletes handle most cleanup via FK constraints
    -- Just delete the user row; profiles, teams, members cascade
    DELETE FROM public.users WHERE id = target_user_id;
END;
$$;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_league_members_league ON public.league_members(league_id);
CREATE INDEX IF NOT EXISTS idx_league_members_user ON public.league_members(user_id);
CREATE INDEX IF NOT EXISTS idx_fantasy_players_team ON public.fantasy_players(team_id);
CREATE INDEX IF NOT EXISTS idx_draft_picks_session ON public.draft_picks(draft_session_id);
CREATE INDEX IF NOT EXISTS idx_player_pool_session ON public.player_pool(draft_session_id);
CREATE INDEX IF NOT EXISTS idx_daily_rankings_league_date ON public.daily_rankings(league_id, date);
CREATE UNIQUE INDEX IF NOT EXISTS idx_fantasy_sleepers_team ON public.fantasy_sleepers(team_id);
