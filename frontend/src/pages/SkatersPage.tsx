import LoadingSpinner from "@/components/common/LoadingSpinner";
import ErrorMessage from "@/components/common/ErrorMessage";
import TopSkatersTable from "@/components/skaters/TopSkatersTable";
import { useSkaters } from "@/features/skaters";

const SkatersPage = () => {
  const {
    filteredSkaters,
    positions,
    isLoading,
    error,
    refetch,
    searchTerm,
    setSearchTerm,
    positionFilter,
    setPositionFilter,
    inPlayoffsFilter,
    setInPlayoffsFilter,
  } = useSkaters();

  if (error) {
    return (
      <div>
        <ErrorMessage
          message="Failed to load skaters data. Please try again."
          onRetry={() => refetch()}
        />
      </div>
    );
  }

  return (
    <div>
      {/* Filters */}
      <div className="bg-[#FACC15]/10 p-4 rounded-none mb-6 border-2 border-[#FACC15]/40">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            {/* Search Filter */}
            <div>
              <label className="block text-xs font-bold uppercase tracking-wider text-[#1A1A1A] mb-1">
                Search
              </label>
              <div className="relative">
                <div className="absolute inset-y-0 left-0 flex items-center pl-3 pointer-events-none">
                  <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                </div>
                <input
                  type="text"
                  placeholder="Search skaters or teams..."
                  value={searchTerm}
                  onChange={(e) => setSearchTerm(e.target.value)}
                  className="w-full pl-10 pr-4 py-2 bg-white border-2 border-[#1A1A1A]/20 rounded-none text-[#1A1A1A] placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-[#FACC15] focus:border-[#FACC15]"
                />
              </div>
            </div>

            {/* Position Filter */}
            <div>
              <label className="block text-xs font-bold uppercase tracking-wider text-[#1A1A1A] mb-1">
                Position
              </label>
              <select
                value={positionFilter}
                onChange={(e) => setPositionFilter(e.target.value)}
                className="w-full p-2 bg-white border-2 border-[#1A1A1A]/20 rounded-none text-[#1A1A1A] focus:outline-none focus:ring-2 focus:ring-[#FACC15] focus:border-[#FACC15]"
              >
                <option value="all">All Positions</option>
                {positions.map((pos) => (
                  <option key={pos} value={pos}>
                    {pos}
                  </option>
                ))}
              </select>
            </div>

            {/* Playoff Status Filter */}
            <div>
              <label className="block text-xs font-bold uppercase tracking-wider text-[#1A1A1A] mb-1">
                Playoff Status
              </label>
              <select
                value={inPlayoffsFilter}
                onChange={(e) => setInPlayoffsFilter(e.target.value)}
                className="w-full p-2 bg-white border-2 border-[#1A1A1A]/20 rounded-none text-[#1A1A1A] focus:outline-none focus:ring-2 focus:ring-[#FACC15] focus:border-[#FACC15]"
              >
                <option value="all">All Skaters</option>
                <option value="in">In Playoffs</option>
                <option value="out">Eliminated</option>
              </select>
            </div>
          </div>
      </div>

      {/* Skaters table */}
      {isLoading ? (
        <LoadingSpinner size="large" message="Loading skaters data..." />
      ) : (
        <TopSkatersTable skaters={filteredSkaters} isLoading={isLoading} />
      )}
    </div>
  );
};

export default SkatersPage;
