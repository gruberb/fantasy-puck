import type { ReactNode } from "react";

interface InsightCardProps {
  title: string;
  /** Header background; any CSS color, including `var(--token)`. */
  accent: string;
  /** Header text color. Light accents (e.g. `--color-you`) need dark ink. */
  ink?: string;
  headerRight?: ReactNode;
  bodyClassName?: string;
  children: ReactNode;
}

export default function InsightCard({
  title,
  accent,
  ink = "#FFFFFF",
  headerRight,
  bodyClassName = "p-6",
  children,
}: InsightCardProps) {
  return (
    <section className="bg-white rounded-none border-2 border-[#1A1A1A] overflow-hidden">
      <header
        className={`px-6 py-3 border-b-2 border-[#1A1A1A]${headerRight ? " flex items-center justify-between" : ""}`}
        style={{ backgroundColor: accent, color: ink }}
      >
        <h2 className="font-extrabold uppercase tracking-wider text-sm">
          {title}
        </h2>
        {headerRight}
      </header>
      <div className={bodyClassName}>{children}</div>
    </section>
  );
}
