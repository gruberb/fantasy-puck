import React from "react";

interface RankingTableEmptyProps {
  message?: string;
}

const RankingTableEmpty: React.FC<RankingTableEmptyProps> = ({
  message = "No rankings data available.",
}) => {
  return (
    <div className="text-center py-8">
      <svg
        className="w-16 h-16 text-gray-300 mx-auto mb-4"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
        xmlns="http://www.w3.org/2000/svg"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={1}
          d="M9.172 16.172a4 4 0 015.656 0M12 14a2 2 0 100-4 2 2 0 000 4z"
        />
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={1}
          d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
      <p className="text-gray-500">{message}</p>
    </div>
  );
};

export default RankingTableEmpty;
