import { ReactNode } from "react";

export type RankingData = Record<string, any>;

export interface Column {
  key: string; // Property name in data or special key like 'rank'
  header: string; // Column header text
  render?: (value: any, row: RankingData, index: number) => ReactNode; // Optional custom renderer
  className?: string; // Optional class for the column
  responsive?: "always" | "md" | "lg"; // When to show the column
  sortable?: boolean; // Whether this column is sortable
}

export interface RankingTableProps {
  // Core data props
  data: RankingData[];
  columns: Column[];
  keyField?: string;
  rankField?: string;

  // Display options
  title?: string;
  subtitle?: string;
  limit?: number;
  viewAllLink?: string;
  viewAllText?: string;
  /** Force-show the top-right link even when `limit` isn't set or all
   *  rows fit. Default behavior only shows it when the rendered list
   *  is truncated. Used by Live Rankings where the link is a
   *  navigational affordance, not an overflow indicator. */
  alwaysShowViewAll?: boolean;
  /** Replaces the default `RankingTableHeader` entirely. Used by
   *  Live Rankings to render a red banner + pulse dot inside the
   *  same outer border as the table body, so the banner and body
   *  read as one card. When set, `title` / `subtitle` / `dateBadge`
   *  / `viewAllLink` are ignored. */
  customHeader?: ReactNode;
  dateBadge?: string | Date;

  // State flags
  isLoading?: boolean;
  emptyMessage?: string;

  // Styling
  className?: string;
  showRankColors?: boolean;

  // Behavior
  initialSortKey?: string;
  initialSortDirection?: "asc" | "desc";

  showDatePicker?: boolean;
  selectedDate?: string;
  onDateChange?: (date: string) => void;
  /** Inclusive lower bound for the date picker (YYYY-MM-DD). Disables
   *  prev-day nav + calendar days before this. */
  minDate?: string;
  /** Inclusive upper bound for the date picker (YYYY-MM-DD). Disables
   *  next-day nav + calendar days after this. */
  maxDate?: string;
}
