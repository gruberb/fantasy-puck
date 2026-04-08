interface GameOptionsProps {
  hasLiveGames: boolean;
  autoRefresh: boolean;
  setAutoRefresh: (value: boolean) => void;
  onRefresh: () => void;
}

export default function GameOptions({
  hasLiveGames,
  autoRefresh,
  setAutoRefresh,
  onRefresh,
}: GameOptionsProps) {
  return (
    <div className="flex items-center gap-3 mb-4">
      {/* Live update toggle */}
      {hasLiveGames && (
        <label htmlFor="autoRefresh" className="flex items-center gap-1.5 text-sm text-gray-700 cursor-pointer">
          <input
            type="checkbox"
            id="autoRefresh"
            checked={autoRefresh}
            onChange={(e) => setAutoRefresh(e.target.checked)}
          />
          Auto-refresh
          {autoRefresh && (
            <span className="h-2 w-2 rounded-full bg-red-500 animate-pulse" title="Every 30s" />
          )}
        </label>
      )}

      {/* Manual refresh button */}
      <button
        onClick={onRefresh}
        className="ml-auto flex items-center gap-1 px-2.5 py-1 text-sm text-[#2563EB] bg-[#2563EB]/10 rounded hover:bg-[#2563EB]/20"
      >
        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        Refresh
      </button>
    </div>
  );
}
