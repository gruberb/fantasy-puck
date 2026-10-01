import { useState, useMemo, useRef, useEffect } from "react";
import { Column, RankingData, RankingTableProps } from "./types";
import RankingTableHeader from "./RankingTableHeader";
import RankingTableEmpty from "./RankingTableEmpty";
import { LoadingSpinner } from "@gruberb/fun-ui";

const RankingTable = <T extends RankingData>({
  // Core data props
  data,
  columns,
  keyField = "id",
  rankField = "rank",

  // Display options
  title,
  subtitle,
  limit,
  viewAllLink,
  viewAllText = "View All",
  alwaysShowViewAll = false,
  customHeader,
  dateBadge,

  // State flags
  isLoading = false,
  emptyMessage = "No data available.",

  // Styling
  className = "",
  showRankColors = true,

  // Behavior
  initialSortKey,
  initialSortDirection = "desc",
  rowClassName,
  stickyHeader = false,

  // Date picker props with defaults
  showDatePicker = false,
  selectedDate,
  onDateChange,
  minDate,
  maxDate,
}: RankingTableProps<T>) => {
  const tableContainerRef = useRef<HTMLDivElement>(null);
  const tableRef = useRef<HTMLTableElement>(null);
  const [isScrollable, setIsScrollable] = useState(false);

  // Check if table is scrollable
  useEffect(() => {
    const checkScrollable = () => {
      if (tableContainerRef.current && tableRef.current) {
        const containerWidth = tableContainerRef.current.clientWidth;
        const tableWidth = tableRef.current.clientWidth;
        setIsScrollable(tableWidth > containerWidth);
      }
    };

    // Check initially
    checkScrollable();

    // Check on window resize
    window.addEventListener("resize", checkScrollable);
    return () => {
      window.removeEventListener("resize", checkScrollable);
    };
  }, []);

  // Set default sort field from the first sortable column or first column
  const defaultSortKey =
    initialSortKey ||
    columns.find((col) => col.sortable)?.key ||
    columns[0]?.key;

  // Sorting state
  const [sortKey, setSortKey] = useState<string>(defaultSortKey);
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">(
    initialSortDirection,
  );

  // Handle sort change
  const handleSort = (key: string) => {
    if (sortKey === key) {
      setSortDirection(sortDirection === "asc" ? "desc" : "asc");
    } else {
      setSortKey(key);
      setSortDirection("desc");
    }
  };

  // Helper to get rank color
  const getRankColor = (rank: number): string => {
    if (!showRankColors) return "rank-indicator rank-indicator-default";

    if (rank === 1) return "rank-indicator rank-indicator-1";
    if (rank === 2) return "rank-indicator rank-indicator-2";
    if (rank === 3) return "rank-indicator rank-indicator-3";
    return "rank-indicator rank-indicator-default";
  };

  // Safely ensure data is an array
  const safeData = useMemo(() => {
    return Array.isArray(data) ? data : [];
  }, [data]);

  // Sort and limit items
  const displayItems = useMemo(() => {
    if (safeData.length === 0) return [];

    // Create a copy for sorting
    let result = [...safeData];

    const direction = sortDirection === "asc" ? 1 : -1;
    result.sort((a, b) => direction * compareValues(a[sortKey], b[sortKey]));

    // Apply limit if specified
    if (limit && limit > 0) {
      result = result.slice(0, limit);
    }

    return result;
  }, [safeData, sortKey, sortDirection, limit]);

  // Find name column (usually the second column after rank)
  const nameColumnIndex = columns.findIndex((col) => col.key !== rankField);
  const hasNameColumn = nameColumnIndex !== -1;

  const renderHeaderLabel = (column: Column<T>) =>
    column.sortable ? (
      <button
        className="focus:outline-none cursor-pointer"
        onClick={() => handleSort(column.key)}
      >
        {column.header}
        {sortKey === column.key && (
          <span className="ml-1">{sortDirection === "asc" ? "↑" : "↓"}</span>
        )}
      </button>
    ) : (
      column.header
    );

  // Sticky rank/name headers sit on the corner of both scroll axes, so
  // they need to stack above the other pinned headers.
  const pinnedHeaderClass = stickyHeader ? "top-0 z-30" : "z-20";
  const scrollHeaderClass = stickyHeader ? "sticky top-0 z-20" : "";
  const showDefaultHeader = !!(
    title ||
    dateBadge ||
    viewAllLink ||
    showDatePicker
  );

  return (
    <div className={`ranking-table-container ${className}`}>
      {/* Header section: caller can pass `customHeader` to replace the
          default bar — Live Rankings does this to slot in a red banner
          + pulse dot inside the same outer border. */}
      {customHeader ? (
        customHeader
      ) : showDefaultHeader && (
        <div className="ranking-table-header">
          <RankingTableHeader
            title={title}
            subtitle={subtitle}
            viewAllLink={viewAllLink}
            viewAllText={viewAllText}
            showViewAll={alwaysShowViewAll || (!!limit && safeData.length > limit)}
            dateBadge={showDatePicker ? undefined : dateBadge}
            showDatePicker={showDatePicker}
            selectedDate={selectedDate}
            onDateChange={onDateChange}
            minDate={minDate}
            maxDate={maxDate}
          />
        </div>
      )}
      {isLoading && (
        <div className="p-6">
          <LoadingSpinner message="Loading data..." />
        </div>
      )}
      {(!safeData || safeData.length === 0) && !isLoading && (
        <div className="p-6">
          <RankingTableEmpty message={emptyMessage} />
        </div>
      )}

      {/* Table */}
      {!isLoading && safeData && safeData.length > 0 && (
        <div>
          <div className="ranking-table-body">
            <div
              ref={tableContainerRef}
              className={
                stickyHeader
                  ? "overflow-auto max-h-[75vh]"
                  : "overflow-x-auto scrollbar-hide"
              }
              style={{ position: "relative" }}
            >
              <table ref={tableRef} className="ranking-table">
                <thead>
                  <tr>
                    {/* Rank column (sticky) */}
                    <th
                      className={`sticky left-0 ${pinnedHeaderClass} bg-[#FACC15]/20 text-center`}
                    >
                      {columns.find((col) => col.key === rankField)?.header ||
                        "Rank"}
                    </th>

                    {/* Name column (sticky if found) */}
                    {hasNameColumn && (
                      <th
                        className={`sticky ${pinnedHeaderClass} border-l border-[#FACC15]/10 sticky-shadow bg-[#FACC15]/20`}
                        style={{ left: "65px" }}
                      >
                        {renderHeaderLabel(columns[nameColumnIndex])}
                      </th>
                    )}

                    {/* Other columns (scrollable) */}
                    {columns
                      .filter(
                        (col, idx) =>
                          col.key !== rankField && idx !== nameColumnIndex,
                      )
                      .map((column) => (
                        <th
                          key={column.key}
                          className={`${scrollHeaderClass} ${responsiveClasses[column.responsive ?? "always"]} ${column.className || ""}`}
                        >
                          {renderHeaderLabel(column)}
                        </th>
                      ))}
                  </tr>
                </thead>
                <tbody>
                  {displayItems.map((item, index) => {
                    const key = item[keyField] ?? index;
                    const rankValue = item[rankField] ?? index + 1;

                    return (
                      <tr
                        key={key}
                        className={`group bg-white hover:bg-[#fef9e7] ${rowClassName?.(item) ?? ""}`}
                      >
                        {/* Rank column (sticky) */}
                        <td
                          className="sticky left-0 z-10 text-center bg-inherit group-hover:bg-[#fef9e7] transition-colors"
                          style={{ width: "50px" }}
>
                          <div className={getRankColor(Number(rankValue))}>
                            {rankValue}
                          </div>
                        </td>

                        {/* Name column (sticky if found) */}
                        {hasNameColumn && (
                          <td
                            className="sticky z-10 border-l border-gray-50 bg-inherit group-hover:bg-[#fef9e7] transition-colors"
                            style={{ left: "65px" }}
                          >
                            {columns[nameColumnIndex].render
                              ? columns[nameColumnIndex].render(
                                  item[columns[nameColumnIndex].key],
                                  item,
                                  index,
                                )
                              : item[columns[nameColumnIndex].key]}
                          </td>
                        )}

                        {/* Other columns (scrollable) */}
                        {columns
                          .filter(
                            (col, idx) =>
                              col.key !== rankField && idx !== nameColumnIndex,
                          )
                          .map((column) => {
                            const value = item[column.key];
                            return (
                              <td
                                key={column.key}
                                className={`${responsiveClasses[column.responsive ?? "always"]} ${column.className || ""}`}
                              >
                                {column.render
                                  ? column.render(value, item, index)
                                  : value}
                              </td>
                            );
                          })}
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
          <style>{`
            .scrollbar-hide {
              -ms-overflow-style: none; /* IE and Edge */
              scrollbar-width: none; /* Firefox */
            }
            .scrollbar-hide::-webkit-scrollbar {
              display: none; /* Chrome, Safari and Opera */
            }
          `}</style>
          {/* Scroll indicator - only show when scrollable */}
          {isScrollable && (
            <div className="table-scroll-indicator">
              <span className="hidden sm:inline">⟷ Scroll for more</span>
              <span className="sm:hidden">⟷ Swipe for more</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

const responsiveClasses: Record<NonNullable<Column["responsive"]>, string> = {
  always: "",
  sm: "hidden sm:table-cell",
  md: "hidden md:table-cell",
  lg: "hidden lg:table-cell",
};

// Missing values rank below every real value so a sparse stat (e.g. TOI
// for a call-up) sinks to the bottom of a descending sort.
function compareValues(a: unknown, b: unknown): number {
  if (a == null || b == null) return a == null ? (b == null ? 0 : -1) : 1;
  if (typeof a === "string" && typeof b === "string") return a.localeCompare(b);
  if (typeof a === "number" && typeof b === "number") return a - b;
  return 0;
}

export default RankingTable;
