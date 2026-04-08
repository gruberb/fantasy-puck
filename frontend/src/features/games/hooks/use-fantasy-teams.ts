import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import { FantasyTeamCount } from "@/types/fantasy";
import { SkaterWithPoints } from "@/types/skaters";

export function useFantasyTeams(selectedDate: string) {
  const { data: gamesData, isLoading: gamesLoading } = useQuery({
    queryKey: ["games", selectedDate],
    queryFn: () => api.getGames(selectedDate),
  });

  // Compute `fantasyTeamCounts` based only on gamesData
  const fantasyTeamCounts: FantasyTeamCount[] = useMemo(() => {
    if (!gamesData?.games) return [];

    // Create a map to track fantasy teams
    const map = new Map<string, FantasyTeamCount>();

    // Helper to process one player list
    const processPlayers = (
      players: SkaterWithPoints[] = [],
      gameId: number,
      nhlTeam: string,
      logo: string,
    ) => {
      players.forEach((p) => {
        const key = p.fantasyTeam;
        if (!key) return;

        if (!map.has(key)) {
          map.set(key, {
            teamId: p.fantasyTeamId,
            teamName: key,
            playerCount: 0,
            players: [],
            totalPoints: 0,
          });
        }

        const entry = map.get(key)!;
        entry.playerCount++;
        const pts = typeof p.points === "number" ? p.points : 0;
        entry.totalPoints += pts;
        entry.players.push({ ...p, gameId, nhlTeam, teamLogo: logo });
      });
    };

    // Walk through each game
    for (const g of gamesData.games) {
      processPlayers(g.homeTeamPlayers, g.id, g.homeTeam, g.homeTeamLogo ?? "");
      processPlayers(g.awayTeamPlayers, g.id, g.awayTeam, g.awayTeamLogo ?? "");
    }

    // Sort players inside each team
    map.forEach((t) =>
      t.players.sort((a, b) => (b.points || 0) - (a.points || 0)),
    );

    // Return only teams with at least one player
    return Array.from(map.values()).filter((t) => t.playerCount > 0);
  }, [gamesData]);

  return {
    gamesData,
    fantasyTeamCounts,
    isLoading: gamesLoading,
    gamesLoading,
  };
}
