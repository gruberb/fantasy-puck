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
}
