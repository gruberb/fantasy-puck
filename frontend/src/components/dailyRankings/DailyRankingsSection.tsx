// components/dailyRankings/DailyRankingsSection.tsx
import { useState } from "react";
import RankingTable from "@/components/common/RankingTable";
import { dailyRankingsColumns } from "@/components/rankingsPageTableColumns/dailysColumns";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import { toLocalDateString, dateStringToLocalDate } from "@/utils/timezone";

const DailyRankingsSection = () => {
  // Local state for the date
  const [selectedDate, setSelectedDate] = useState<string>(() => {
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    return toLocalDateString(yesterday);
  });

  // Get daily rankings for the selected date
  const {
    data: dailyRankings,
    isLoading: dailyRankingsLoading,
    error: dailyRankingsError,
  } = useQuery({
    queryKey: ["dailyRankings", selectedDate],
    queryFn: () => api.getDailyFantasySummary(selectedDate),
    retry: 1,
  });

  // Process the daily rankings data
  let processedDailyRankings = [];
  if (dailyRankings) {
    if (Array.isArray(dailyRankings)) {
      processedDailyRankings = dailyRankings;
    } else if (
      typeof dailyRankings === "object" &&
      "rankings" in dailyRankings
    ) {
      processedDailyRankings = dailyRankings.rankings;
    } else if (
      typeof dailyRankings === "object" &&
      "data" in dailyRankings &&
      dailyRankings.data &&
      typeof dailyRankings.data === "object" &&
      "rankings" in dailyRankings.data
    ) {
      processedDailyRankings = dailyRankings.data.rankings;
    }
  }

  // Format display date
  const displayDate = dateStringToLocalDate(selectedDate);

  return (
    <div>
      <div className="bg-white rounded-none p-4 mb-4 flex justify-between items-center">
        <h2 className="text-xl font-bold">Daily Fantasy Scores</h2>

        <div className="flex items-center space-x-2">
          {/* Simple date navigation controls */}
          <button
            onClick={() => {
              const date = dateStringToLocalDate(selectedDate);
              date.setDate(date.getDate() - 1);
              setSelectedDate(toLocalDateString(date));
            }}
            className="p-2 bg-gray-100 hover:bg-gray-200 rounded-none"
          >
            <svg
              className="w-5 h-5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M15 19l-7-7 7-7"
              />
            </svg>
          </button>

          <div className="bg-gray-100 px-3 py-2 rounded-none text-sm font-medium">
            {displayDate.toLocaleDateString("en-US", {
              weekday: "short",
              month: "short",
              day: "numeric",
              year: "numeric",
            })}
          </div>

          <button
            onClick={() => {
              const date = dateStringToLocalDate(selectedDate);
              date.setDate(date.getDate() + 1);
              setSelectedDate(toLocalDateString(date));
            }}
            className="p-2 bg-gray-100 hover:bg-gray-200 rounded-none"
          >
            <svg
              className="w-5 h-5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9 5l7 7-7 7"
              />
            </svg>
          </button>

          <button
            onClick={() => {
              const yesterday = new Date();
              yesterday.setDate(yesterday.getDate() - 1);
              setSelectedDate(toLocalDateString(yesterday));
            }}
            className="text-sm px-3 py-2 bg-gray-100 hover:bg-gray-200 rounded-none"
          >
            Yesterday
          </button>
        </div>
      </div>

      <RankingTable
        columns={dailyRankingsColumns}
        data={processedDailyRankings}
        keyField="teamId"
        rankField="rank"
        title="Daily Fantasy Scores"
        isLoading={dailyRankingsLoading}
        emptyMessage={"No daily rankings available for this date"}
        dateBadge={displayDate}
        initialSortKey="dailyPoints"
        initialSortDirection="desc"
      />
    </div>
  );
};

export default DailyRankingsSection;
