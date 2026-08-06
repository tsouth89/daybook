import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { createPortal } from "react-dom";

export type MenuItem =
  | {
      kind?: "item";
      label: string;
      shortcut?: string;
      disabled?: boolean;
      danger?: boolean;
      onClick: () => void;
    }
  | { kind: "sep" };

type Pos = { x: number; y: number };

type Props = {
  pos: Pos | null;
  items: MenuItem[];
  onClose: () => void;
};

/** App-wide right-click menu. Render once near the portal root via state from useContextMenu. */
export function ContextMenu({ pos, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [fixed, setFixed] = useState<Pos | null>(null);

  useLayoutEffect(() => {
    if (!pos || !ref.current) {
      setFixed(pos);
      return;
    }
    const el = ref.current;
    const rect = el.getBoundingClientRect();
    let x = pos.x;
    let y = pos.y;
    if (x + rect.width > window.innerWidth - 8) x = window.innerWidth - rect.width - 8;
    if (y + rect.height > window.innerHeight - 8) y = window.innerHeight - rect.height - 8;
    if (x < 8) x = 8;
    if (y < 8) y = 8;
    setFixed({ x, y });
  }, [pos, items]);

  useEffect(() => {
    if (!pos) return;
    const close = () => onClose();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("scroll", close, true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", close);
    };
  }, [pos, onClose]);

  if (!pos) return null;

  const point = fixed ?? pos;

  return createPortal(
    <div
      ref={ref}
      className="ctxmenu"
      style={{ left: point.x, top: point.y }}
      onMouseDown={(e) => e.stopPropagation()}
      role="menu"
    >
      {items.map((item, i) => {
        if (item.kind === "sep") {
          return <div key={`sep-${i}`} className="ctxmenu-sep" />;
        }
        return (
          <button
            key={`${item.label}-${i}`}
            type="button"
            role="menuitem"
            className={`ctxmenu-item ${item.danger ? "danger" : ""}`}
            disabled={item.disabled}
            onClick={() => {
              if (item.disabled) return;
              onClose();
              item.onClick();
            }}
          >
            <span>{item.label}</span>
            {item.shortcut && <span className="ctxmenu-shortcut">{item.shortcut}</span>}
          </button>
        );
      })}
    </div>,
    document.body
  );
}

export function useContextMenu() {
  const [pos, setPos] = useState<Pos | null>(null);
  const [items, setItems] = useState<MenuItem[]>([]);

  function open(e: ReactMouseEvent, next: MenuItem[]) {
    e.preventDefault();
    e.stopPropagation();
    setItems(next.filter((it) => it.kind === "sep" || it.label));
    setPos({ x: e.clientX, y: e.clientY });
  }

  function close() {
    setPos(null);
  }

  return { pos, items, open, close, menuProps: { pos, items, onClose: close } };
}

export async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}
