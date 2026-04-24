export interface TeamConcentrationCell {
  nhlTeam: string;
  rostered: number;
  teamPlayoffPoints: number;
}

export interface TeamYesterdayPlayerLine {
  name: string;
  nhlTeam: string;
  goals: number;
  assists: number;
  points: number;
}

export interface TeamYesterdayTeamLine {
  teamId: number;
  teamName: string;
  goals: number;
  assists: number;
  points: number;
}

export interface TeamYesterdaySummary {
  date: string;
  nhlGames: number;
  completedGames: number;
  myGoals: number;
  myAssists: number;
  myPoints: number;
  myPlayers: TeamYesterdayPlayerLine[];
  leagueTopThreeSource: "yesterday" | "playoff_total";
  leagueTopThree: TeamYesterdayTeamLine[];
}

export interface TeamDiagnosis {
  headline: string;
  narrativeMarkdown: string;
  leagueRank: number;
  leagueSize: number;
  gapToFirst: number;
  gapToThird: number;
  yesterday: TeamYesterdaySummary;
  concentrationByTeam: TeamConcentrationCell[];
}
