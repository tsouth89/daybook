import { useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { api, errText, type TrashItem } from "./api";
import ConfirmDialog from "./ConfirmDialog";

type Props = {
  open: boolean;
  onClose: () => void;
  onRestored: () => void;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

function describe(item: TrashItem): string {
  switch (item.payload.kind) {
    case "Entry":
      return item.payload.record.kind;
    case "Entity":
      return item.payload.entity_kind;
    case "Inbox":
      return "capture";
  }
}

/** Undo, for the deletions that used to be final. */
export default function Trash({ open, onClose, onRestored, onError, onNotice }: Props) {
  const [items, setItems] = useState<TrashItem[]>([]);
  const [confirmEmpty, setConfirmEmpty] = useState(false);

  const load = useCallback(async () => {
    try {
      setItems(await api.listTrash());
    } catch (e) {
      onError(errText(e));
    }
  }, [onError]);

  useEffect(() => {
    if (open) load();
  }, [open, load]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  async function restore(id: string) {
    try {
      const what = await api.restoreTrash(id);
      onNotice?.(`Restored ${what}`);
      await load();
      onRestored();
    } catch (e) {
      onError(errText(e));
      await load();
    }
  }

  async function purge(id: string) {
    try {
      await api.purgeTrash(id);
      await load();
    } catch (e) {
      onError(errText(e));
    }
  }

  return createPortal(
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div className="modal trash-modal" onMouseDown={(e) => e.stopPropagation()} role="dialog">
        <h3>Trash</h3>
        {items.length === 0 ? (
          <p className="dim">Nothing deleted. Anything you remove lands here first.</p>
        ) : (
          <ul className="trash-list">
            {items.map((i) => (
              <li key={i.id}>
                <div className="trash-main">
                  <span className="trash-label">{i.label || "(untitled)"}</span>
                  <span className="pill">{describe(i)}</span>
                </div>
                <span className="dim tiny mono">{i.deleted_at.replace("T", " ")}</span>
                <button className="btn tiny-btn trash-btn" onClick={() => void restore(i.id)}>
                  Restore
                </button>
                <button
                  className="linkbtn tiny"
                  onClick={() => void purge(i.id)}
                  title="Delete permanently"
                >
                  forget
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="modal-actions">
          {items.length > 0 && (
            <button className="btn danger" onClick={() => setConfirmEmpty(true)}>
              Empty trash
            </button>
          )}
          <span className="grow" />
          <button className="btn primary" onClick={onClose}>
            Done
          </button>
        </div>
      </div>
      <ConfirmDialog
        open={confirmEmpty}
        title="Empty the trash?"
        body="Everything in here is deleted for good. There is no second undo."
        confirmLabel="Empty it"
        danger
        onCancel={() => setConfirmEmpty(false)}
        onConfirm={() => {
          setConfirmEmpty(false);
          void api
            .emptyTrash()
            .then((n) => {
              onNotice?.(`Emptied ${n} item${n === 1 ? "" : "s"}`);
              return load();
            })
            .catch((e) => onError(errText(e)));
        }}
      />
    </div>,
    document.body
  );
}
