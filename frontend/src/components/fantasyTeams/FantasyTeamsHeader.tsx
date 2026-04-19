import { PageHeader } from "@gruberb/fun-ui";

interface FantasyTeamsHeaderProps {
  searchTerm: string;
  setSearchTerm: (term: string) => void;
}

export default function FantasyTeamsHeader({
  searchTerm,
  setSearchTerm,
}: FantasyTeamsHeaderProps) {
  return (
    <PageHeader
      title="Fantasy Teams"
      subtitle="View and manage your fantasy hockey teams"
    >
      <div className="relative max-w-md w-64">
        <div className="absolute inset-y-0 left-0 flex items-center pl-3 pointer-events-none">
          <svg
            className="w-4 h-4 text-gray-400"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>
        </div>
        <input
          type="text"
          placeholder="Search teams..."
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
          className="w-full pl-10 pr-4 py-2 bg-white border-2 border-[#1A1A1A]/30 rounded-none text-[#1A1A1A] placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-[#2563EB]"
        />
      </div>
    </PageHeader>
  );
}
