import type { Column } from "@/components/common/RankingTable/types";
import type { CalibrationRoundReport } from "@/features/admin/types";

const fixed4 = (value: number) => value.toFixed(4);

/** Column set for the per-round table in the admin Calibrate summary. */
export const calibrationRoundsColumns: Column<CalibrationRoundReport>[] = [
  { key: "round", header: "Round" },
  { key: "games_scored", header: "Games", className: "tabular-nums" },
  { key: "brier", header: "Brier", className: "tabular-nums", render: fixed4 },
  { key: "log_loss", header: "Log-loss", className: "tabular-nums", render: fixed4 },
];
