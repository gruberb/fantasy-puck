import { FantasyTeamPoints } from "@/types/fantasyTeams";

interface TeamStatsProps {
  teamPoints: FantasyTeamPoints;
}

export default function TeamStats({ teamPoints }: TeamStatsProps) {
  return (
    <section className="card">
      <h2 className="text-2xl font-bold mb-4">Team Stats</h2>
      <div className="space-y-4">
        <div className="flex justify-between items-center border-b pb-2">
          <span className="font-medium">Goals</span>
          <span className="font-bold text-xl">
            {teamPoints.teamTotals.goals}
          </span>
        </div>
        <div className="flex justify-between items-center border-b pb-2">
          <span className="font-medium">Assists</span>
          <span className="font-bold text-xl">
            {teamPoints.teamTotals.assists}
          </span>
        </div>
        <div className="flex justify-between items-center border-b pb-2">
          <span className="font-medium">Total Points</span>
          <span className="font-bold text-xl">
            {teamPoints.teamTotals.totalPoints}
          </span>
        </div>
        <div className="grid grid-cols-2 gap-4 mt-4">
          <div className="bg-blue-50 p-4 rounded-none text-center">
            <div className="text-sm font-medium mb-1">Goals</div>
            <div className="text-3xl font-bold">
              {teamPoints.teamTotals.goals}
            </div>
          </div>
          <div className="bg-green-50 p-4 rounded-none text-center">
            <div className="text-sm font-medium mb-1">Assists</div>
            <div className="text-3xl font-bold">
              {teamPoints.teamTotals.assists}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
