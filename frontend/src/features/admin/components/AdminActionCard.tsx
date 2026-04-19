import type { ReactNode } from "react";
import { ResultPanel } from "./ResultPanel";

interface AdminActionCardProps {
  title: string;
  description: string;
  /** Inputs or controls rendered above the Run button. Optional. */
  children?: ReactNode;
  runLabel?: string;
  onRun: () => void;
  disabled?: boolean;
  data: unknown;
  error: string | null;
  ranAt: Date | null;
  isPending: boolean;
  /** Optional extra summary rendered between the Run button and the
   *  raw JSON pane — used by Calibrate to show a Brier/log-loss table. */
  summary?: ReactNode;
}

export function AdminActionCard({
  title,
  description,
  children,
  runLabel = "Run",
  onRun,
  disabled,
  data,
  error,
  ranAt,
  isPending,
  summary,
}: AdminActionCardProps) {
  return (
    <div className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
      <div className="px-5 py-3 bg-[#1A1A1A] text-white">
        <h3 className="font-extrabold uppercase tracking-wider text-sm">
          {title}
        </h3>
      </div>
      <div className="p-5 space-y-3">
        <p className="text-xs text-gray-600 leading-relaxed">{description}</p>
        {children && <div className="space-y-2">{children}</div>}
        <div>
          <button
            type="button"
            onClick={onRun}
            disabled={disabled || isPending}
            className="px-4 py-2 bg-[#FACC15] border-2 border-[#1A1A1A] text-[#1A1A1A] font-extrabold uppercase tracking-wider text-xs hover:bg-[#1A1A1A] hover:text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-[#FACC15] disabled:hover:text-[#1A1A1A]"
          >
            {isPending ? "Running…" : runLabel}
          </button>
        </div>
        {summary && ranAt && !error && <div>{summary}</div>}
        <ResultPanel data={data} error={error} ranAt={ranAt} isPending={isPending} />
      </div>
    </div>
  );
}
