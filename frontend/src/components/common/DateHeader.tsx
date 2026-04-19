import { useState, useRef, useEffect } from "react";
import DatePicker from "react-datepicker";
import "react-datepicker/dist/react-datepicker.css";
import {
  toLocalDateString,
  dateStringToLocalDate,
  formatDisplayDate,
} from "@/utils/timezone";

interface DateHeaderProps {
  selectedDate: string;
  onDateChange: (date: string) => void;
  isFloating?: boolean;
  /** Inclusive lower bound (YYYY-MM-DD). Disables prev nav + calendar dates before this. */
  minDate?: string;
  /** Inclusive upper bound (YYYY-MM-DD). Disables next nav + calendar dates after this. */
  maxDate?: string;
}

const DateHeader = ({
  selectedDate,
  onDateChange,
  isFloating = true,
  minDate,
  maxDate,
}: DateHeaderProps) => {
  const [isCalendarOpen, setIsCalendarOpen] = useState(false);
  const [isVisible, setIsVisible] = useState(true);
  const [lastScrollY, setLastScrollY] = useState(0);
  const datePickerRef = useRef<HTMLDivElement>(null);

  const date = dateStringToLocalDate(selectedDate);
  const minBound = minDate ? dateStringToLocalDate(minDate) : null;
  const maxBound = maxDate ? dateStringToLocalDate(maxDate) : null;
  const atOrBeforeMin = minBound
    ? date.getTime() <= minBound.getTime()
    : false;
  const atOrAfterMax = maxBound
    ? date.getTime() >= maxBound.getTime()
    : false;
  const formattedDisplayDate = formatDisplayDate(date, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });

  const handleDateChange = (newDate: Date | null) => {
    if (newDate) {
      onDateChange(toLocalDateString(newDate));
      setIsCalendarOpen(false);
    }
  };

  const handlePrevDay = () => {
    if (atOrBeforeMin) return;
    const prevDate = new Date(date);
    prevDate.setDate(date.getDate() - 1);
    onDateChange(toLocalDateString(prevDate));
  };

  const handleNextDay = () => {
    if (atOrAfterMax) return;
    const nextDate = new Date(date);
    nextDate.setDate(date.getDate() + 1);
    onDateChange(toLocalDateString(nextDate));
  };

  // "Today" within the allowed window: clamp to the nearest bound when
  // the real today is outside (e.g. before playoffs start, or after the
  // season ends). Without the clamp the button would navigate to an
  // out-of-window date that then fails the picker's minDate/maxDate.
  const handleToday = () => {
    const today = new Date();
    let target = today;
    if (minBound && today.getTime() < minBound.getTime()) target = minBound;
    if (maxBound && today.getTime() > maxBound.getTime()) target = maxBound;
    onDateChange(toLocalDateString(target));
  };

  useEffect(() => {
    if (!isFloating) return;
    const handleScroll = () => {
      const currentScrollY = window.scrollY;
      setIsVisible(currentScrollY <= 0 || currentScrollY < lastScrollY);
      setLastScrollY(currentScrollY);
    };
    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, [isFloating, lastScrollY]);

  useEffect(() => {
    if (!isCalendarOpen) return;
    const handleOutsideClick = (e: MouseEvent) => {
      if (datePickerRef.current && !datePickerRef.current.contains(e.target as Node)) {
        setIsCalendarOpen(false);
      }
    };
    document.addEventListener("mousedown", handleOutsideClick);
    return () => document.removeEventListener("mousedown", handleOutsideClick);
  }, [isCalendarOpen]);

  return (
    <div
      className={`${
        isFloating
          ? "sticky top-16 z-30 transition-all duration-100 bg-white border-2 border-[#1A1A1A] rounded-none"
          : ""
      } ${isFloating && !isVisible ? "-translate-y-full opacity-0" : ""}`}
    >
      <div className="flex items-center justify-between p-3">
        <div className="text-sm font-bold uppercase tracking-wider">{formattedDisplayDate}</div>

        <div className="flex items-center gap-2" ref={datePickerRef}>
          <button
            onClick={handlePrevDay}
            disabled={atOrBeforeMin}
            className="w-8 h-8 flex items-center justify-center bg-white border-2 border-[#1A1A1A] text-[#1A1A1A] font-bold hover:bg-[#1A1A1A] hover:text-white transition-colors duration-100 disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-white disabled:hover:text-[#1A1A1A]"
            aria-label="Previous day"
          >
            &lt;
          </button>

          <div className="relative">
            <button
              onClick={() => setIsCalendarOpen(!isCalendarOpen)}
              className="px-3 py-1.5 bg-white border-2 border-[#1A1A1A] text-[#1A1A1A] font-bold uppercase tracking-wider text-xs hover:bg-[#1A1A1A] hover:text-white transition-colors duration-100 flex items-center"
            >
              {date.toLocaleDateString("en-US", { month: "short", day: "numeric" })}
              <span className={`ml-1 inline-block transition-transform ${isCalendarOpen ? "rotate-180" : ""}`}>
                ▼
              </span>
            </button>

            {isCalendarOpen && (
              <div className="absolute right-0 mt-1 z-50 border-2 border-[#1A1A1A] bg-white">
                <DatePicker
                  selected={date}
                  onChange={handleDateChange}
                  minDate={minBound ?? undefined}
                  maxDate={maxBound ?? undefined}
                  inline
                  showMonthDropdown
                  showYearDropdown
                  dropdownMode="select"
                />
              </div>
            )}
          </div>

          <button
            onClick={handleNextDay}
            disabled={atOrAfterMax}
            className="w-8 h-8 flex items-center justify-center bg-white border-2 border-[#1A1A1A] text-[#1A1A1A] font-bold hover:bg-[#1A1A1A] hover:text-white transition-colors duration-100 disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-white disabled:hover:text-[#1A1A1A]"
            aria-label="Next day"
          >
            &gt;
          </button>

          <button
            onClick={handleToday}
            className="px-3 py-1.5 bg-[#FACC15] border-2 border-[#1A1A1A] text-[#1A1A1A] font-bold uppercase tracking-wider text-xs hover:bg-[#1A1A1A] hover:text-white transition-colors duration-100"
          >
            Today
          </button>
        </div>
      </div>
    </div>
  );
};

export default DateHeader;
