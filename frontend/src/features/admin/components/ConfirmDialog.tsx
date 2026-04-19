import { Modal } from "@gruberb/fun-ui";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  body: string;
  confirmLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Small wrapper around fun-ui's `Modal` specialised for destructive or
 * long-running admin actions. Keeps the existing call-site prop shape
 * (open/body/confirmLabel/onConfirm/onCancel) so the admin panels
 * don't care about the underlying modal primitive.
 */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel = "Confirm",
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  return (
    <Modal
      isOpen={open}
      onClose={onCancel}
      title={title}
      footer={
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
      }
    >
      <p className="text-sm text-[#1A1A1A] leading-relaxed">{body}</p>
    </Modal>
  );
}
