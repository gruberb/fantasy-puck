import { useState } from "react";

interface ResultPanelProps {
  data: unknown;
  error: string | null;
  ranAt: Date | null;
  isPending: boolean;
}

/**
 * Pretty-prints an admin action's JSON response with a copy button
 * and a timestamp. Green border on success, red on error. Collapsed
 * until the first run.
 */
export function ResultPanel({ data, error, ranAt, isPending }: ResultPanelProps) {
  const [copied, setCopied] = useState(false);

  if (isPending) {
    return (
      <div className="mt-3 border-2 border-gray-200 bg-gray-50 px-4 py-3 text-xs font-bold uppercase tracking-wider text-gray-500">
        Running…
      </div>
    );
  }

  if (!ranAt) return null;

  const ok = error === null;
  const payload = ok ? data : { error };
  const pretty = JSON.stringify(payload, null, 2);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(pretty);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* ignored — nav/clipboard can fail under iframe or insecure contexts */
    }
  };

  return (
    <div
      className={`mt-3 border-2 ${
        ok ? "border-emerald-500" : "border-red-500"
      } bg-white overflow-hidden`}
    >
      <div
        className={`flex items-center justify-between px-3 py-1.5 text-[10px] uppercase tracking-widest font-bold ${
          ok ? "bg-emerald-50 text-emerald-700" : "bg-red-50 text-red-700"
        }`}
      >
        <span>{ok ? "OK" : "Error"} · {ranAt.toLocaleTimeString()}</span>
        <button
          type="button"
          onClick={copy}
          className="underline decoration-dotted hover:opacity-70"
        >
          {copied ? "Copied" : "Copy JSON"}
        </button>
      </div>
      <pre className="p-3 text-[11px] leading-relaxed overflow-auto max-h-80 text-[#1A1A1A] whitespace-pre-wrap break-words">
        {pretty}
      </pre>
    </div>
  );
}
