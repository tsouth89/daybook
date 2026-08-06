import {
  createContext,
  useContext,
  useEffect,
  useRef,
  type ReactNode,
  type RefObject,
} from "react";

/** Hooks a view exposes to the shell (global shortcuts, navigate-away guard). */
export type ViewHandlers = {
  /** True while an editor holds unsaved changes. */
  isDirty?: () => boolean;
  /** Ctrl+Shift+P — process this view's pending inbox items. */
  process?: () => void;
};

const HostContext = createContext<RefObject<ViewHandlers> | null>(null);

export function ViewHostProvider({
  handlers,
  children,
}: {
  handlers: RefObject<ViewHandlers>;
  children: ReactNode;
}) {
  return <HostContext.Provider value={handlers}>{children}</HostContext.Provider>;
}

/**
 * Register this view's handlers with the shell. Only one view is mounted at a
 * time, so the shell keeps a single slot; the latest render's closures are used.
 */
export function useViewHandlers(h: ViewHandlers) {
  const slot = useContext(HostContext);
  const latest = useRef(h);
  latest.current = h;
  const canProcess = !!h.process;
  const canDirty = !!h.isDirty;

  useEffect(() => {
    if (!slot) return;
    slot.current = {
      isDirty: canDirty ? () => latest.current.isDirty?.() ?? false : undefined,
      process: canProcess ? () => latest.current.process?.() : undefined,
    };
    return () => {
      slot.current = {};
    };
  }, [slot, canProcess, canDirty]);
}
