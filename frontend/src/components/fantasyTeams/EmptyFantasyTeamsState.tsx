export default function EmptyFantasyTeamsState({ onRetry }: { onRetry?: () => void }) {
  return (
    <div className="bg-white rounded-none p-12 text-center border border-gray-100">
      <div className="relative">
        {/* Decorative background elements */}
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-32 h-32 bg-[#2563EB]/5 rounded-none"></div>
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-24 h-24 bg-[#2563EB]/5 rounded-none animate-pulse"></div>

        {/* Icon with animation */}
        <svg
          className="w-20 h-20 text-[#2563EB]/30 mx-auto mb-6 relative"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M9.172 16.172a4 4 0 015.656 0M12 14a2 2 0 100-4 2 2 0 000 4z"
            className="animate-pulse"
          />
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
      </div>

      <p className="text-gray-500 text-lg">No teams match your search.</p>

      <div className="mt-6">
        <button
          onClick={() => onRetry?.()}
          className="inline-flex items-center px-4 py-2 bg-[#2563EB]/10 hover:bg-[#2563EB]/20 text-[#2563EB] rounded-none transition-colors"
        >
          <svg
            className="w-4 h-4 mr-2"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
            />
          </svg>
          Reset Search
        </button>
      </div>
    </div>
  );
}
