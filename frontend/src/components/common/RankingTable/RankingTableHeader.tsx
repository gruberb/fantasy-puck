// components/common/RankingTable/RankingTableHeader.tsx
import React, { useState, useRef } from "react";
import { Link } from "react-router-dom";
import DatePicker from "react-datepicker";
import "react-datepicker/dist/react-datepicker.css";
import {
  formatDisplayDate,
  toLocalDateString,
  dateStringToLocalDate,
  getHockeyDateYesterday,
} from "@/utils/timezone";

interface RankingTableHeaderProps {
  title?: string;
  subtitle?: string;
  viewAllLink?: string;
  viewAllText?: string;
  showViewAll?: boolean;
  dateBadge?: string | Date;

  // Date picker props
  showDatePicker?: boolean;
  selectedDate?: string;
  onDateChange?: (date: string) => void;
  /** Inclusive YYYY-MM-DD lower bound. Disables prev nav + calendar
   *  dates before this so the picker never exposes pre-playoff dates. */
  minDate?: string;
  /** Inclusive YYYY-MM-DD upper bound. Same contract as minDate for
   *  the end of the playoff window. */
  maxDate?: string;
}

const RankingTableHeader: React.FC<RankingTableHeaderProps> = ({
  title,
  subtitle,
  viewAllLink,
  viewAllText = "View All",
  showViewAll = false,
  dateBadge,

  // DatePicker props with defaults
  showDatePicker = false,
  selectedDate,
  onDateChange,
  minDate,
  maxDate,
}) => {
  const [isCalendarOpen, setIsCalendarOpen] = useState(false);
  const datePickerRef = useRef<HTMLDivElement>(null);

  if (!title && !dateBadge && !viewAllLink && !showDatePicker) return null;

  // Format date badge
  const formattedDate =
    showDatePicker && selectedDate
      ? formatDisplayDate(dateStringToLocalDate(selectedDate), {
          year: "numeric",
          month: "short",
          day: "numeric",
        })
      : typeof dateBadge === "string"
        ? dateBadge
        : dateBadge instanceof Date
          ? formatDisplayDate(dateBadge, {
              year: "numeric",
              month: "short",
              day: "numeric",
            })
          : "";

  // Handle date change
  const handleDateChange = (newDate: Date | null) => {
    if (newDate && onDateChange) {
      onDateChange(toLocalDateString(newDate));
      setIsCalendarOpen(false);
    }
  };

  // YYYY-MM-DD strings compare lexicographically, so string bounds
  // save us a round-trip through Date objects.
  const atOrBeforeMin =
    !!(minDate && selectedDate && selectedDate <= minDate);
  const atOrAfterMax =
    !!(maxDate && selectedDate && selectedDate >= maxDate);

  // Handle previous and next day
  const handlePrevDay = () => {
    if (atOrBeforeMin) return;
    if (selectedDate && onDateChange) {
      const date = dateStringToLocalDate(selectedDate);
      date.setDate(date.getDate() - 1);
      onDateChange(toLocalDateString(date));
    }
  };

  const handleNextDay = () => {
    if (atOrAfterMax) return;
    if (selectedDate && onDateChange) {
      const date = dateStringToLocalDate(selectedDate);
      date.setDate(date.getDate() + 1);
      onDateChange(toLocalDateString(date));
    }
  };

  // Handle going to yesterday. Clamp to bounds so the button doesn't
  // kick the picker to an out-of-window date on day 1 of the playoffs.
  const handleYesterday = () => {
    if (!onDateChange) return;
    let target = getHockeyDateYesterday();
    if (minDate && target < minDate) target = minDate;
    if (maxDate && target > maxDate) target = maxDate;
    onDateChange(target);
  };

  // Custom datepicker portal to avoid z-index issues
  const CustomDatePickerContainer = ({
    children,
  }: {
    children: React.ReactNode;
  }) => {
    return (
      <div
        style={{
          position: "absolute",
          zIndex: 9999,
          top: 20,
          right: -235,
          marginTop: "8px",
          boxShadow: "0 2px 10px rgba(0, 0, 0, 0.1)",
          borderRadius: "8px",
          overflow: "hidden",
        }}
      >
        {children}
      </div>
    );
  };

  return (
    <div className="flex flex-col md:flex-row justify-between items-start md:items-center space-y-3 md:space-y-0">
      {/* Left side - Title and badge */}
      <div>
        {title && <h2 className="text-xl font-bold">{title}</h2>}
        {subtitle && <p className="text-sm opacity-90">{subtitle}</p>}

        {/* Always show date badge */}
        {(formattedDate || (showDatePicker && selectedDate)) && (
          <div className="inline-flex items-center mt-1">
            <span className="bg-[#FACC15] text-[#1A1A1A] text-xs px-3 py-1 rounded-none font-medium">
              {formattedDate ||
                (selectedDate &&
                  formatDisplayDate(dateStringToLocalDate(selectedDate)))}
            </span>
          </div>
        )}
      </div>

      {/* Right side with controls */}
      <div className="flex flex-wrap items-center gap-2">
        {/* Date picker controls */}
        {showDatePicker && (
          <div
            ref={datePickerRef}
            className="relative flex flex-wrap items-center gap-2"
          >
            {/* Date navigation */}
            <div className="flex items-center gap-2">
              <button
                onClick={handlePrevDay}
                disabled={atOrBeforeMin}
                className="w-8 h-8 flex items-center justify-center bg-white border-2 border-[#1A1A1A] text-[#1A1A1A] font-bold hover:bg-[#1A1A1A] hover:text-white transition-colors duration-100 disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-white disabled:hover:text-[#1A1A1A]"
                aria-label="Previous day"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  className="h-4 w-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M15 19l-7-7 7-7"
                  />
                </svg>
              </button>

              {/* Date display button */}
              <button
                className="px-3 py-1.5 bg-white border-2 border-[#1A1A1A] text-[#1A1A1A] font-bold uppercase tracking-wider text-xs hover:bg-[#1A1A1A] hover:text-white transition-colors duration-100 focus:outline-none flex items-center"
                onClick={() => setIsCalendarOpen(!isCalendarOpen)}
              >
                <span className="mr-1">
                  {selectedDate
                    ? dateStringToLocalDate(selectedDate).toLocaleDateString(
                        "en-US",
                        {
                          month: "short",
                          day: "numeric",
                          year: "numeric",
                        },
                      )
                    : "Select date"}
                </span>
                <span
                  className={`inline-block transition-transform ${isCalendarOpen ? "rotate-180" : ""}`}
                >
                  ▼
                </span>
              </button>

              {/* Custom date picker that renders in a portal */}
              {isCalendarOpen && (
                <div className="absolute w-0 h-0">
                  <CustomDatePickerContainer>
                    <DatePicker
                      selected={
                        selectedDate
                          ? dateStringToLocalDate(selectedDate)
                          : null
                      }
                      onChange={handleDateChange}
                      minDate={minDate ? dateStringToLocalDate(minDate) : undefined}
                      maxDate={maxDate ? dateStringToLocalDate(maxDate) : undefined}
                      inline
                      showMonthDropdown
                      showYearDropdown
                      dropdownMode="select"
                      onClickOutside={() => setIsCalendarOpen(false)}
                    />
                  </CustomDatePickerContainer>
                </div>
              )}

              <button
                onClick={handleNextDay}
                disabled={atOrAfterMax}
                className="w-8 h-8 flex items-center justify-center bg-white border-2 border-[#1A1A1A] text-[#1A1A1A] font-bold hover:bg-[#1A1A1A] hover:text-white transition-colors duration-100 disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-white disabled:hover:text-[#1A1A1A]"
                aria-label="Next day"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  className="h-4 w-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M9 5l7 7-7 7"
                  />
                </svg>
              </button>
            </div>

            {/* Quick navigation buttons */}
            <button
              onClick={handleYesterday}
              className="px-3 py-1.5 bg-[#FACC15] border-2 border-[#1A1A1A] text-[#1A1A1A] font-bold uppercase tracking-wider text-xs hover:bg-[#1A1A1A] hover:text-white transition-colors duration-100"
            >
              Yesterday
            </button>
          </div>
        )}

        {/* View All link */}
        {showViewAll && viewAllLink && (
          <Link
            to={viewAllLink}
            className="text-yellow-300 hover:text-yellow-200 flex items-center font-medium transition-colors"
          >
            {viewAllText} <span className="ml-1">→</span>
          </Link>
        )}
      </div>
    </div>
  );
};

export default RankingTableHeader;
