export interface TeamConcentrationCell {
  nhlTeam: string;
  rostered: number;
  teamPlayoffPoints: number;
}

export interface TeamDiagnosis {
  headline: string;
  narrativeMarkdown: string;
  leagueRank: number;
  leagueSize: number;
  gapToFirst: number;
  gapToThird: number;
  concentrationByTeam: TeamConcentrationCell[];
}
