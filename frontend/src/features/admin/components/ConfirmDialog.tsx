interface ConfirmDialogProps {
  open: boolean;
  title: string;
  body: string;
  confirmLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Minimal modal for destructive / long-running admin actions. Styled
 * to match the brutalist palette: thick borders, yellow primary,
 * red-bordered warning frame on the dialog body.
 */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel = "Confirm",
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-50 bg-black/50 flex items-center justify-center p-4"
      onClick={onCancel}
    >
      <div
        className="bg-white border-2 border-[#EF4444] shadow-[6px_6px_0px_0px_#1A1A1A] max-w-md w-full"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="bg-[#EF4444] text-white px-5 py-3">
          <h3 className="font-extrabold uppercase tracking-wider text-sm">
            {title}
          </h3>
        </div>
        <div className="p-5 space-y-4">
          <p className="text-sm text-[#1A1A1A] leading-relaxed">{body}</p>
          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={onCancel}
              className="px-4 py-2 border-2 border-[#1A1A1A] text-[#1A1A1A] bg-white font-extrabold uppercase tracking-wider text-xs hover:bg-[#1A1A1A] hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={onConfirm}
              className="px-4 py-2 border-2 border-[#1A1A1A] bg-[#EF4444] text-white font-extrabold uppercase tracking-wider text-xs hover:bg-[#1A1A1A] transition-colors"
            >
              {confirmLabel}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
