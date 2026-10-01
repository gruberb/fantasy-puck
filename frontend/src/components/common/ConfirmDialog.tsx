import type { ReactNode } from "react";
import { Button, Modal } from "@gruberb/fun-ui";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  body?: ReactNode;
  /** Rendered above `body`, e.g. a preview of the player being picked. */
  children?: ReactNode;
  confirmLabel?: string;
  confirmVariant?: "danger" | "primary";
  confirmDisabled?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** fun-ui `Modal` with a Cancel / Confirm footer, for any action that needs an explicit yes. */
export function ConfirmDialog({
  open,
  title,
  body,
  children,
  confirmLabel = "Confirm",
  confirmVariant = "danger",
  confirmDisabled = false,
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
          <Button type="button" variant="secondary" size="sm" onClick={onCancel}>
            Cancel
          </Button>
          <Button
            type="button"
            variant={confirmVariant}
            size="sm"
            onClick={onConfirm}
            disabled={confirmDisabled}
          >
            {confirmLabel}
          </Button>
        </div>
      }
    >
      <div className="space-y-4">
        {children}
        {body && <p className="text-sm text-[#1A1A1A] leading-relaxed">{body}</p>}
      </div>
    </Modal>
  );
}
