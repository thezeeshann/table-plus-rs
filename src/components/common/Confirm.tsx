import { Modal } from "./Modal";

export function Confirm({ title, message, confirmLabel = "OK", danger, onConfirm, onCancel, children }: {
  title: string; message?: string; confirmLabel?: string; danger?: boolean; onConfirm: () => void; onCancel: () => void; children?: React.ReactNode;
}) {
  return (
    <Modal
      title={title}
      onClose={onCancel}
      footer={
        <>
          <button className="btn" onClick={onCancel}>Cancel</button>
          <button className={`btn ${danger ? "btn-danger" : "btn-primary"}`} onClick={onConfirm} autoFocus>{confirmLabel}</button>
        </>
      }
    >
      {message && <p className="whitespace-pre-wrap">{message}</p>}
      {children}
    </Modal>
  );
}
