import { useState } from "react";
import { api, errText, type AskAnswer } from "../api";
import { useFormat } from "../FormatContext";
import Markdown from "../Markdown";
import { useNavigate } from "../nav";

const EXAMPLES = [
  "What's outstanding on my projects?",
  "What did I decide about auth?",
  "What did I get done last week?",
];

/**
 * Recall over the item layer. Retrieval runs on routed entries rather than raw
 * days, so a question pulls the handful of entries that mention it instead of
 * whole days of unrelated content.
 */
export default function AskView({
  vaultPath,
  onError,
}: {
  vaultPath: string;
  onError: (m: string) => void;
}) {
  const [question, setQuestion] = useState("");
  const [asked, setAsked] = useState("");
  const [result, setResult] = useState<AskAnswer | null>(null);
  const [busy, setBusy] = useState(false);
  const fmt = useFormat();
  const navigate = useNavigate();

  async function ask(q?: string) {
    const text = (q ?? question).trim();
    if (!text || busy) return;
    setBusy(true);
    setResult(null);
    setAsked(text);
    if (q) setQuestion(q);
    try {
      setResult(await api.askVault(text));
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="content askview">
      <div className="ask-bar">
        <input
          autoFocus
          placeholder="Ask your daybook…"
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void ask();
          }}
          spellCheck={false}
        />
        <button
          className="btn primary"
          onClick={() => void ask()}
          disabled={busy || !question.trim()}
        >
          {busy ? "Thinking…" : "Ask"}
        </button>
      </div>

      {!result && !busy && (
        <div className="ask-examples">
          <span className="dim tiny">Try:</span>
          {EXAMPLES.map((e) => (
            <button key={e} className="chip clickable" onClick={() => void ask(e)}>
              {e}
            </button>
          ))}
        </div>
      )}

      {busy && <p className="dim tiny pad">Reading your entries…</p>}

      {result && (
        <>
          <div className="ask-answer">
            <div className="dim tiny">{asked}</div>
            <Markdown text={result.answer} vaultPath={vaultPath} />
          </div>

          <h3 className="section-label pad-top">
            Drawn from ({result.used.length})
          </h3>
          <p className="dim tiny">
            Answers come only from these entries. If something looks wrong, this is where to
            check it.
          </p>
          <ul className="recent-list">
            {result.used.map((e) => (
              <li key={e.id}>
                <button
                  className="linkbtn"
                  onClick={() =>
                    e.slug
                      ? navigate({
                          type: "entity",
                          kind: e.kind === "area" ? "area" : "project",
                          slug: e.slug,
                        })
                      : navigate({ type: "day", date: e.date, pane: "note" })
                  }
                >
                  {e.title || "(untitled)"}
                </button>
                <span className="dim tiny">
                  {fmt.date(e.date)} · {e.kind}
                  {e.slug ? ` · ${e.name || e.slug}` : ""}
                </span>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
