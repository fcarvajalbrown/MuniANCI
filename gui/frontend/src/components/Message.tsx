import { useState } from "react";
import type { Citation, DisambiguationCategory, SearchResult } from "../api";

export interface UIMessage {
  id: number;
  role: "user" | "assistant";
  content: string;
  citations?: Citation[];
  webResults?: SearchResult[];
  streaming?: boolean;
  error?: boolean;
  // Deterministic disambiguation chips (see main.py `_is_ambiguous`) and the
  // original vague query they resolve, so a chip click can resend it.
  categories?: DisambiguationCategory[];
  pendingMessage?: string;
}

// De-duplicates citations by source for a compact display, keeping the set of
// chunk indices per source (FR-03 / FR-12: show source filename + chunk).
function groupCitations(citations: Citation[]): { source: string; chunks: number[] }[] {
  const map = new Map<string, number[]>();
  for (const c of citations) {
    const list = map.get(c.source) ?? [];
    if (!list.includes(c.chunk_index)) list.push(c.chunk_index);
    map.set(c.source, list);
  }
  return [...map.entries()].map(([source, chunks]) => ({
    source,
    chunks: chunks.sort((a, b) => a - b),
  }));
}

const PENSANDO = "Pensando";

function LetrasPensando() {
  return (
    <span className="pensando-letras" aria-label="Pensando">
      {[...PENSANDO].map((letra, i) => (
        <span key={i} style={{ animationDelay: `${i * 0.08}s` }} aria-hidden="true">
          {letra}
        </span>
      ))}
      <span aria-hidden="true">...</span>
    </span>
  );
}

export interface MessageProps {
  msg: UIMessage;
  onSelectCategory?: (categoryId: string) => void;
}

export function Message({ msg, onSelectCategory }: MessageProps) {
  const [abierto, setAbierto] = useState(false);
  const grouped = msg.citations ? groupCitations(msg.citations) : [];
  const hayProceso = msg.streaming || grouped.length > 0;

  return (
    <div className={`msg msg-${msg.role}${msg.error ? " msg-error" : ""}`}>
      <div className="msg-role">{msg.role === "user" ? "Tú" : "Asistente"}</div>

      <div className="msg-body">
        {msg.content}
        {msg.streaming && <span className="cursor" aria-hidden="true">▋</span>}
      </div>

      {msg.categories && msg.categories.length > 0 && (
        <div className="category-chips" role="group" aria-label="Categorías de trámite">
          {msg.categories.map((c) => (
            <button
              key={c.id}
              type="button"
              className="category-chip"
              onClick={() => onSelectCategory?.(c.id)}
            >
              {c.label}
            </button>
          ))}
        </div>
      )}

      {hayProceso && (
        <div className="proceso">
          <button
            type="button"
            className={`proceso-toggle${abierto ? " abierto" : ""}`}
            aria-expanded={abierto}
            onClick={() => setAbierto((v) => !v)}
          >
            <span className="proceso-flecha" aria-hidden="true">
              {abierto ? "▾" : "▸"}
            </span>
            {msg.streaming ? <LetrasPensando /> : <span>Cómo llegué a esto</span>}
          </button>

          {abierto && (
            <div className="proceso-detalle">
              {grouped.length === 0 ? (
                <p className="proceso-vacio">Buscando en el corpus legal de este equipo...</p>
              ) : (
                <>
                  <p className="proceso-nota">
                    Lo que el buscador encontró y le pasó al modelo. No es la lista de
                    normas en que se apoya la respuesta: puede haber traído documentos que
                    no venían al caso. Las referencias que la respuesta afirma van en su
                    propio texto.
                  </p>
                  <ul>
                    {grouped.map((g) => (
                      <li key={g.source}>
                        <span className="cite-source">{g.source}</span>
                        <span className="cite-chunks">
                          {" "}
                          (fragmento{g.chunks.length > 1 ? "s" : ""} {g.chunks.join(", ")})
                        </span>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </div>
          )}
        </div>
      )}

      {msg.webResults && msg.webResults.length > 0 && (
        <div className="web-results" aria-label="Resultados web">
          <div className="citations-title">Resultados web</div>
          <ul>
            {msg.webResults.map((r, i) => (
              <li key={i}>
                <a href={r.url} target="_blank" rel="noreferrer">
                  {r.title}
                </a>
                {r.snippet && <div className="web-snippet">{r.snippet}</div>}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
