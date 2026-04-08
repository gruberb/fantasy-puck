// ── Draft Feature Types ───────────────────────────────────────────────────
// Backend sends camelCase (serde rename_all = "camelCase"). All fields are camelCase.

export interface LeagueMember {
  id: string;
  leagueId?: string;
  userId: string;
  fantasyTeamId: number;
  draftOrder: number;
  displayName?: string;
  teamName?: string;
}

export interface DraftSession {
  id: string;
  leagueId: string;
  status: 'pending' | 'active' | 'paused' | 'completed' | 'picks_done';
  currentRound: number;
  currentPickIndex: number;
  totalRounds: number;
  snakeDraft: boolean;
  startedAt?: string | null;
  completedAt?: string | null;
  sleeperStatus: 'active' | 'completed' | null;
  sleeperPickIndex: number;
}

export interface PlayerPoolEntry {
  id: string;
  draftSessionId?: string;
  nhlId: number;
  name: string;
  position: string;
  nhlTeam: string;
  headshotUrl: string;
}

export interface DraftPick {
  id: string;
  draftSessionId: string;
  leagueMemberId: string;
  playerPoolId: string;
  nhlId: number;
  playerName: string;
  nhlTeam: string;
  position: string;
  round: number;
  pickNumber: number;
  pickedAt?: string;
}

// ── Snake Draft Helper ────────────────────────────────────────────────────

export function getPickerForPick(
  pickNumber: number,
  members: LeagueMember[],
  snakeDraft: boolean,
): LeagueMember | undefined {
  if (members.length === 0) return undefined;
  const round = Math.floor(pickNumber / members.length);
  const indexInRound = pickNumber % members.length;
  const isReversed = snakeDraft && round % 2 === 1;
  return isReversed
    ? members[members.length - 1 - indexInRound]
    : members[indexInRound];
}
