export interface NhlTeamRosterRow {
  nhlTeam: string;
  teamName: string;
  teamLogo: string;
  rosteredCount: number;
  playoffPoints: number;
  topSkaterName: string | null;
  topSkaterPhoto: string | null;
  topSkaterPoints: number | null;
}

export interface RosteredSkaterRow {
  nhlId: number;
  name: string;
  photo: string;
  nhlTeam: string;
  teamLogo: string;
  playoffPoints: number;
  fantasyTeamId: number;
  fantasyTeamName: string;
}

export interface LeagueStatsResponse {
  nhlTeamsRostered: NhlTeamRosterRow[];
  topRosteredSkaters: RosteredSkaterRow[];
}
