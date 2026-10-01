import type { Flash } from "@/hooks/use-flash";

const toneClasses = {
  success: "border-[#16A34A] shadow-[4px_4px_0px_0px_#16A34A]",
  error: "border-[#EF4444] shadow-[4px_4px_0px_0px_#EF4444]",
} as const;

/** Bottom-centre status toast; pair with `useFlash()`. */
export function Toast({ flash }: { flash: Flash | null }) {
  if (!flash) return null;
  return (
    <div
      role={flash.tone === "error" ? "alert" : "status"}
      className={`fixed bottom-6 left-1/2 -translate-x-1/2 z-50 bg-[#1A1A1A] text-white px-6 py-3 border-2 text-sm font-bold uppercase tracking-wider max-w-lg text-center ${toneClasses[flash.tone]}`}
    >
      {flash.message}
    </div>
  );
}
