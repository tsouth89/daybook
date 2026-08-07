import type { InboxProcessResult } from "./api";
import { destinationToNav, useNavigate } from "./nav";

/** Result banner for a process run; destinations are clickable where they map to a view. */
export default function ProcessResult({
  result,
  label = "Processed",
}: {
  result: InboxProcessResult;
  label?: string;
}) {
  const navigate = useNavigate();

  const chips: { label: string; date: string }[] = [];
  for (const p of result.processed) {
    for (const d of p.destinations) {
      if (!chips.some((c) => c.label === d)) chips.push({ label: d, date: p.date });
    }
  }
  const entries = result.processed.reduce((a, p) => a + p.entry_count, 0);
  const actions = result.processed.flatMap((p) => p.actions ?? []);
  const created = [...new Set(result.processed.flatMap((p) => p.new_entities))];
  const n = result.processed.length;

  return (
    <div className={`banner ${result.errors.length ? "warn" : "ok"}`}>
      <div>
        {label} {n} item{n === 1 ? "" : "s"}
        {entries > 0 && ` · ${entries} entr${entries === 1 ? "y" : "ies"}`}
        {created.length > 0 && (
          <>
            {" "}
            · New: <strong>{created.join(", ")}</strong>
          </>
        )}
        {result.errors.length > 0 && (
          <>
            {" "}
            · {result.errors.length} failed (still in inbox): {result.errors.join(" · ")}
          </>
        )}
      </div>
      {actions.length > 0 && (
        <div className="did">
          {actions.map((a, i) => (
            <div key={i}>
              <span className="did-mark">*</span> {a}
            </div>
          ))}
        </div>
      )}
      {chips.length > 0 && (
        <div className="chips">
          {chips.map((c) => {
            const target = destinationToNav(c.label, c.date);
            return target ? (
              <button
                key={c.label}
                type="button"
                className="chip clickable"
                onClick={() => navigate(target)}
                title="Open destination"
              >
                {c.label}
              </button>
            ) : (
              <span key={c.label} className="chip">
                {c.label}
              </span>
            );
          })}
        </div>
      )}
    </div>
  );
}
