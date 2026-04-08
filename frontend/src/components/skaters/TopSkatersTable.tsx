import React, { useState, useRef, useEffect } from "react";
import { createPortal } from "react-dom";
import { Link } from "react-router-dom";
import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";
import { TopSkater } from "@/types/skaters";
import { usePlayoffsData } from "@/features/rankings";
import { useLeague } from "@/contexts/LeagueContext";

interface TopSkatersTableProps {
  skaters: TopSkater[];
  isLoading: boolean;
}

type SortField =
  | "points"
  | "goals"
  | "assists"
  | "plusMinus"
  | "penaltyMins"
  | "faceoffPct"
  | "toi"
  | "lastName";

const TopSkatersTable: React.FC<TopSkatersTableProps> = ({
  skaters,
  isLoading,
}) => {
  const [sortField, setSortField] = useState<SortField>("points");
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("desc");
  const { isTeamInPlayoffs } = usePlayoffsData();
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";
  const tableRef = useRef<HTMLTableElement>(null);
  const tableContainerRef = useRef<HTMLDivElement>(null);
  const headerRef = useRef<HTMLDivElement>(null);

  // Portal state
  const [showFixedHeader, setShowFixedHeader] = useState(false);
  const [tableWidth, setTableWidth] = useState(0);
  const [tableLeft, setTableLeft] = useState(0);
  const [navbarHeight, setNavbarHeight] = useState(64);
  const [headerHeight, setHeaderHeight] = useState(46); // Default height
  const [columnWidths, setColumnWidths] = useState<number[]>([]);
  const [isLargeScreen, setIsLargeScreen] = useState(false);

  // Initialize measurements and check screen size
  useEffect(() => {
    // Check if screen is large
    const checkScreenSize = () => {
      setIsLargeScreen(window.innerWidth >= 1220);
    };

    // Find navbar height once on mount
    const navbar =
      document.querySelector("header") ||
      document.querySelector("nav") ||
      document.querySelector(".navbar");
    if (navbar) {
      setNavbarHeight(navbar.getBoundingClientRect().height);
    }

    // Initial table measurements
    if (tableRef.current) {
      // Measure the header height
      const header = tableRef.current.querySelector("thead");
      if (header) {
        setHeaderHeight(header.getBoundingClientRect().height);
      }

      // Calculate column widths
      const headerCells = tableRef.current.querySelectorAll("thead th");
      const widths = Array.from(headerCells).map(
        (cell) => cell.getBoundingClientRect().width,
      );
      setColumnWidths(widths);

      // Calculate table width
      setTableWidth(tableRef.current.getBoundingClientRect().width);
    }

    // Initial check
    checkScreenSize();

    // Add listener for resize
    window.addEventListener("resize", checkScreenSize);

    // Cleanup
    return () => {
      window.removeEventListener("resize", checkScreenSize);
    };
  }, []);

  // Optimized horizontal scroll synchronization with direct scrollLeft approach
  useEffect(() => {
    if (!tableContainerRef.current) return;

    const container = tableContainerRef.current;
    let rafId = null;
    let ticking = false;
    let lastScrollLeft = -1;

    // Get and store header reference (but will look it up again during scroll)
    let headerEl = document.getElementById("fixed-header-portal");

    // Apply optimizations to header if it exists
    if (headerEl) {
      headerEl.style.overflowX = "auto";
      headerEl.style.overflowY = "hidden";
      headerEl.style.webkitOverflowScrolling = "touch";
    }

    // Optimized scroll handler with requestAnimationFrame
    const syncScroll = () => {
      // Re-get reference to header (it might be recreated by React)
      headerEl = document.getElementById("fixed-header-portal");

      // Only update if scroll position changed and header exists
      if (headerEl && lastScrollLeft !== container.scrollLeft) {
        // Use direct scrollLeft - the most reliable approach
        headerEl.scrollLeft = container.scrollLeft;
        lastScrollLeft = container.scrollLeft;
      }

      ticking = false;
    };

    // Throttled scroll handler
    const handleScroll = () => {
      if (!ticking) {
        ticking = true;
        rafId = requestAnimationFrame(syncScroll);
      }
    };

    // Use passive event listener for better performance
    container.addEventListener("scroll", handleScroll, { passive: true });

    // Initial sync
    requestAnimationFrame(syncScroll);

    // Apply performance optimizations to container
    if (container) {
      container.style.webkitOverflowScrolling = "touch";
    }

    return () => {
      // Clean up
      container.removeEventListener("scroll", handleScroll);
      if (rafId) cancelAnimationFrame(rafId);
    };
  }, []);

  // Track scroll position and handle fixed header visibility
  useEffect(() => {
    if (!tableRef.current || !tableContainerRef.current) return;

    // Measure header only once to avoid recalculation on each scroll
    if (headerHeight === 46 && tableRef.current) {
      const header = tableRef.current.querySelector("thead");
      if (header) {
        setHeaderHeight(header.getBoundingClientRect().height);
      }
    }

    const handleScroll = () => {
      const containerRect = tableContainerRef.current?.getBoundingClientRect();

      if (containerRect) {
        // Update table position
        setTableLeft(containerRect.left);

        // Get table width from the ref
        if (
          tableRef.current &&
          Math.abs(tableWidth - tableRef.current.offsetWidth) > 5
        ) {
          setTableWidth(tableRef.current.offsetWidth);
        }

        // Show fixed header when table top is above navbar
        if (containerRect.top < navbarHeight) {
          if (!showFixedHeader) {
            setShowFixedHeader(true);
          }
        } else {
          if (showFixedHeader) {
            setShowFixedHeader(false);
          }
        }
      }
    };

    // Set up event listeners
    window.addEventListener("scroll", handleScroll, { passive: true });
    window.addEventListener("resize", handleScroll, { passive: true });

    // Initial check
    handleScroll();

    // Cleanup
    return () => {
      window.removeEventListener("scroll", handleScroll);
      window.removeEventListener("resize", handleScroll);
    };
  }, [navbarHeight, showFixedHeader, tableWidth, headerHeight]);

  const handleSort = (field: SortField) => {
    if (field === sortField) {
      setSortDirection(sortDirection === "asc" ? "desc" : "asc");
    } else {
      setSortField(field);
      setSortDirection("desc"); // Default to desc for new field
    }
  };

  const formatTOI = (seconds: number): string => {
    if (seconds == null) return "-";
    const totalSeconds = Math.round(seconds);
    const minutes = Math.floor(totalSeconds / 60);
    const remainingSeconds = totalSeconds % 60;
    return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
  };

  const sortedSkaters = [...skaters].sort((a, b) => {
    let comparison = 0;
    const aValue = a.stats?.[sortField as keyof TopSkater["stats"]];
    const bValue = b.stats?.[sortField as keyof TopSkater["stats"]];

    if (sortField === "lastName") {
      comparison = `${a.lastName}, ${a.firstName}`.localeCompare(
        `${b.lastName}, ${b.firstName}`,
      );
    } else if (aValue == null && bValue == null) {
      comparison = 0;
    } else if (aValue == null) {
      comparison = -1;
    } else if (bValue == null) {
      comparison = 1;
    } else {
      comparison = (aValue as number) - (bValue as number);
    }

    return sortDirection === "asc" ? comparison : -comparison;
  });

  const getSortIcon = (field: SortField) => {
    if (field !== sortField) return null;
    const iconClass = "w-3 h-3 ml-1 inline-block";

    return sortDirection === "asc" ? (
      <svg className={iconClass} fill="currentColor" viewBox="0 0 20 20">
        <path
          fillRule="evenodd"
          d="M5.293 7.707a1 1 0 010-1.414l4-4a1 1 0 011.414 0l4 4a1 1 0 01-1.414 1.414L11 5.414V17a1 1 0 11-2 0V5.414L6.707 7.707a1 1 0 01-1.414 0z"
          clipRule="evenodd"
        ></path>
      </svg>
    ) : (
      <svg className={iconClass} fill="currentColor" viewBox="0 0 20 20">
        <path
          fillRule="evenodd"
          d="M14.707 12.293a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 111.414-1.414L9 14.586V3a1 1 0 012 0v11.586l2.293-2.293a1 1 0 011.414 0z"
          clipRule="evenodd"
        ></path>
      </svg>
    );
  };

  // Generate table header row - used in both the main table and portal
  const renderTableHeader = () => (
    <tr className="border-b border-gray-200 bg-gray-50">
      {/* Rank column */}
      <th
        className="py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-center sticky left-0 z-30 bg-gray-50"
        style={{ width: "2rem" }}
      >
        #
      </th>

      {/* Player column */}
      <th
        className="py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-left sticky left-12 z-30 bg-gray-50"
        style={{ width: "7rem" }}
      >
        <button
          className="flex items-center focus:outline-none cursor-pointer"
          onClick={() => handleSort("lastName")}
        >
          Skater {getSortIcon("lastName")}
        </button>
      </th>

      {/* Team column */}
      <th
        className="py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-left"
        style={{ width: columnWidths[2] || "3rem" }}
      >
        Team
      </th>

      {/* Position column */}
      <th
        className="py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-left items-center"
        style={{ width: columnWidths[3] || "3rem" }}
      >
        Pos
      </th>

      {/* Points column */}
      <th
        className="bg-sky-200/50 py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-center"
        style={{ width: columnWidths[4] || "5rem" }}
      >
        <button
          className="flex items-center justify-center mx-auto focus:outline-none cursor-pointer"
          onClick={() => handleSort("points")}
        >
          Points {getSortIcon("points")}
        </button>
      </th>

      {/* Goals column */}
      <th
        className="bg-sky-100/75 py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-center"
        style={{ width: columnWidths[5] || "4rem" }}
      >
        <button
          className="flex items-center justify-center mx-auto focus:outline-none cursor-pointer"
          onClick={() => handleSort("goals")}
        >
          Goals {getSortIcon("goals")}
        </button>
      </th>

      {/* Assists column */}
      <th
        className="bg-sky-100/75 py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-center"
        style={{ width: columnWidths[6] || "5rem" }}
      >
        <button
          className="flex items-center justify-center mx-auto focus:outline-none cursor-pointer"
          onClick={() => handleSort("assists")}
        >
          Assists {getSortIcon("assists")}
        </button>
      </th>

      {/* Plus/Minus column */}
      <th
        className="py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-center"
        style={{ width: columnWidths[7] || "4rem" }}
      >
        <button
          className="flex items-center justify-center mx-auto focus:outline-none cursor-pointer"
          onClick={() => handleSort("plusMinus")}
        >
          +/- {getSortIcon("plusMinus")}
        </button>
      </th>

      {/* PIM column */}
      <th
        className="py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-center"
        style={{ width: columnWidths[8] || "2rem" }}
      >
        <button
          className="flex items-center justify-center mx-auto focus:outline-none cursor-pointer"
          onClick={() => handleSort("penaltyMins")}
        >
          PIM {getSortIcon("penaltyMins")}
        </button>
      </th>

      {/* TOI column */}
      <th
        className="py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-center"
        style={{ width: columnWidths[9] || "4rem" }}
      >
        <button
          className="flex items-center justify-center mx-auto focus:outline-none cursor-pointer"
          onClick={() => handleSort("toi")}
        >
          TOI {getSortIcon("toi")}
        </button>
      </th>

      {/* Fantasy column */}
      <th
        className="py-4 px-5 whitespace-nowrap text-sm font-semibold tracking-wider text-center"
        style={{ width: columnWidths[10] || "5rem" }}
      >
        Fantasy
      </th>
    </tr>
  );

  // Create the fixed header portal with hidden scrollbar
  const fixedHeaderPortal = createPortal(
    <div
      id="fixed-header-portal"
      ref={headerRef}
      className="fixed scrollbar-hide"
      style={{
        top: `${navbarHeight}px`,
        left: isLargeScreen ? tableLeft : "0px",
        width: isLargeScreen ? tableWidth : "100vw",
        zIndex: 50,
        opacity: showFixedHeader ? 1 : 0,
        pointerEvents: showFixedHeader ? "auto" : "none",
        overflowX: "auto",
        overflowY: "hidden",
        maxWidth: "100vw",
      }}
    >
      <table
        className="w-full border-collapse"
        style={{ width: tableWidth, tableLayout: "fixed" }}
      >
        <thead>{renderTableHeader()}</thead>
      </table>
    </div>,
    document.body,
  );

  if (isLoading) {
    return (
      <div className="bg-white rounded-none p-6 text-center border border-gray-100">
        <div className="animate-pulse">
          <div className="h-6 rounded w-1/4 mb-4 mx-auto"></div>
          <div className="h-4 rounded w-full mb-2.5"></div>
          <div className="h-4 rounded w-full mb-2.5"></div>
          <div className="h-4 rounded w-full mb-2.5"></div>
          <div className="h-4 rounded w-full mb-2.5"></div>
          <div className="h-4 rounded w-full mb-2.5"></div>
        </div>
      </div>
    );
  }

  return (
    <>
      {/* The fixed header portal is always rendered but conditionally visible */}
      {fixedHeaderPortal}

      {/* Main table container */}
      <div
        ref={tableContainerRef}
        className="border border-gray-200 rounded-none overflow-x-auto -mx-4 lg:mx-0 lg:w-full"
        style={{
          width: isLargeScreen ? "100%" : "calc(100% + 2rem)",
          maxWidth: isLargeScreen ? "100%" : "calc(100% + 2rem)",
          marginLeft: isLargeScreen ? "0" : "-1rem",
          marginRight: isLargeScreen ? "0" : "-1rem",
        }}
      >
        <table
          ref={tableRef}
          className="w-full border-collapse min-w-[800px]"
          style={{ tableLayout: "fixed" }}
        >
          <thead style={{ visibility: showFixedHeader ? "hidden" : "visible" }}>
            {renderTableHeader()}
          </thead>
          <tbody>
            {sortedSkaters.map((player, index) => {
              const isInPlayoffs = isTeamInPlayoffs(player.teamAbbrev);

              return (
                <tr
                  key={`${player.id}-${index}`}
                  className={`${!isInPlayoffs ? "opacity-25" : ""} border-b`}
                >
                  {/* Rank column - sticky left */}
                  <td className="text-sm bg-white py-4 px-5 text-center font-medium border-b border-gray-100 sticky left-0 z-10">
                    {index + 1}
                  </td>

                  {/* Player column - sticky left */}
                  <td className="text-sm bg-white py-4 px-5 text-left border-b border-gray-100 sticky left-12 z-10">
                    <div className="flex items-center">
                      <div className="ml-0">
                        <a
                          href={`https://www.nhl.com/player/${player.id}`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="font-medium text-gray-900 hover:underline block"
                        >
                          {player.firstName} {player.lastName}
                        </a>
                      </div>
                    </div>
                  </td>

                  {/* Team column */}
                  <td className="bg-white py-4 px-5 border-b border-gray-100">
                    <a
                      href={`https://www.nhl.com/${getNHLTeamUrlSlug(player.teamAbbrev)}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center group"
                    >
                      <span className="text-sm text-gray-900 group-hover:underline">
                        {player.teamAbbrev}
                      </span>
                    </a>
                  </td>

                  {/* Position column */}
                  <td className="text-sm bg-white py-4 px-5 border-b border-gray-100">
                    {player.position}
                  </td>

                  {/* Points column */}
                  <td className="text-sm bg-sky-200/50 py-4 px-5 text-center font-bold border-b border-gray-100">
                    {player.stats.points ?? "-"}
                  </td>

                  {/* Goals column */}
                  <td className="text-sm bg-sky-100/75 py-4 px-5 text-center border-b border-gray-100">
                    {player.stats.goals ?? "-"}
                  </td>

                  {/* Assists column */}
                  <td className="text-sm bg-sky-100/75 py-4 px-5 text-center border-b border-gray-100">
                    {player.stats.assists ?? "-"}
                  </td>

                  {/* Plus/Minus column */}
                  <td className="bg-white text-sm py-4 px-5 text-center border-b border-gray-100">
                    {player.stats.plusMinus != null ? (
                      <span
                        className={
                          player.stats.plusMinus > 0
                            ? "text-green-600"
                            : player.stats.plusMinus < 0
                              ? "text-red-600"
                              : ""
                        }
                      >
                        {player.stats.plusMinus > 0 ? "+" : ""}
                        {player.stats.plusMinus}
                      </span>
                    ) : (
                      "-"
                    )}
                  </td>

                  {/* PIM column */}
                  <td className="bg-white text-sm py-4 px-5 text-center border-b border-gray-100">
                    {player.stats.penaltyMins ?? 0}
                  </td>

                  {/* TOI column */}
                  <td className="bg-white text-sm py-4 px-5 text-center border-b border-gray-100">
                    {formatTOI(player.stats.toi as number)}
                  </td>

                  {/* Fantasy column */}
                  <td className="bg-white py-4 px-5 text-center whitespace-nowrap border-b border-gray-100">
                    {player.fantasyTeam ? (
                      <Link
                        to={`${lp}/teams/${player.fantasyTeam.teamId}`}
                        className="text-sm text-[#2563EB] hover:underline"
                      >
                        {player.fantasyTeam.teamName}
                      </Link>
                    ) : (
                      <span className="text-sm text-gray-500">—</span>
                    )}
                  </td>
                </tr>
              );
            })}
            {sortedSkaters.length === 0 && !isLoading && (
              <tr>
                <td
                  colSpan={11}
                  className="text-center py-10 px-5 text-gray-500 bg-white"
                >
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
                  No skaters found matching your criteria.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Add styles to handle hover states for sticky columns and hide scrollbars */}
      <style jsx global>{`
        /* Hide scrollbar for Chrome, Safari and Opera */
        .scrollbar-hide::-webkit-scrollbar {
          display: none;
        }

        /* Hide scrollbar for IE, Edge and Firefox */
        .scrollbar-hide {
          -ms-overflow-style: none; /* IE and Edge */
          scrollbar-width: none; /* Firefox */
        }

        /* Fix hover states for sticky columns */
        tr:hover td[style*="position: sticky"] {
          background-color: #eff6ff !important;
        }

        /* Make sure table container takes full width on large screens */
        @media (min-width: 1220px) {
          .lg\\:w-full {
            width: 100% !important;
            margin-left: 0 !important;
            margin-right: 0 !important;
          }
        }

        /* Force hardware acceleration on elements that need it */
        #fixed-header-portal {
          backface-visibility: hidden;
          will-change: transform;
        }

        /* Fixed width column approach */
        th:first-child,
        td:first-child {
          width: 3rem !important;
          min-width: 3rem !important;
          max-width: 3rem !important;
        }

        th:nth-child(2),
        td:nth-child(2) {
          width: 7rem !important;
          border-right: 1px solid #e0e0e0 !important;
        }

        tr td {
          border-bottom: 1px solid #e0e0e0 !important;
        }
      `}</style>
    </>
  );
};

export default TopSkatersTable;
