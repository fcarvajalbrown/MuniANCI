// Asistente tab — hosts the MuniGPT chat inside the MuniANCI window.
//
// The RAG backend runs as a Tauri sidecar (see gui/src/assistant.rs). Readiness
// is resolved from the backend's own /status payload so the UI never spins
// indefinitely:
//
//   - not shipped in this build  -> say so immediately (the host knows at startup;
//                                   waiting out the budget would tell the user
//                                   "it crashed" about something never installed)
//   - reachable + ready          -> render the chat
//   - reachable + NOT ready      -> show the concrete blocker at once (which model
//                                   file is missing, or the missing engine binary)
//                                   plus the two ways to obtain it, and keep
//                                   polling so it self-heals — no endless spinner
//   - not reachable within the
//     startup budget             -> show a "backend failed to start" error
//
// Once /status answers even once, the backend is alive, so we never fall back to
// the "failed to start" state after that.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  fetchConfig,
  fetchStatus,
  setApiBase,
  type AppConfig,
  type BackendStatus,
} from "../api";
import { Chat } from "./Chat";
import { ObtenerModelos } from "./ObtenerModelos";
import "../assistant.css";

interface AssistantStatus {
  running: boolean;
  ready: boolean;
  apiBase: string;
  installed: boolean;
}

type Phase = "starting" | "ready" | "blocked" | "failed" | "not-installed";

// How long to wait for the backend to first answer /status before declaring it
// failed to start. Once it answers, this no longer applies.
const STARTUP_BUDGET_MS = 45_000;
const POLL_MS = 2_000;

function blockedReason(s: BackendStatus): string {
  const missing = s.missingModels ?? [];
  if (missing.length > 0) {
    const plural = missing.length > 1;
    return (
      `Falta${plural ? "n" : ""} ${plural ? "los modelos" : "el modelo"} ` +
      `${missing.join(", ")}. Colóca${plural ? "los" : "lo"} en backend/models/ ` +
      `para activar el Asistente.`
    );
  }
  if (s.serverBinary === false) {
    return "Falta el motor de inferencia (llama-server) en backend/bin/.";
  }
  return "El asistente no está listo todavía.";
}

export function AsistenteTab() {
  const [phase, setPhase] = useState<Phase>("starting");
  const [reason, setReason] = useState("");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [corpusNacional, setCorpusNacional] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    let unlistenTimeout: (() => void) | undefined;
    let everReachable = false;
    let startedAt = Date.now();

    const tick = async () => {
      if (cancelled) return;
      try {
        const s = await fetchStatus();
        everReachable = true;
        if (cancelled) return;
        if (s.ready) {
          setCorpusNacional(s.corpusInstitucional === false);
          setPhase("ready");
          fetchConfig()
            .then((c) => !cancelled && setConfig(c))
            .catch(() => {
              /* config is optional; the chat still works with defaults */
            });
          return; // ready is terminal — stop polling
        }
        // Reachable but blocked: surface why, immediately. Keep polling so it
        // recovers automatically if the missing file is dropped in.
        setReason(blockedReason(s));
        setPhase("blocked");
        timer = window.setTimeout(tick, POLL_MS);
      } catch {
        if (cancelled) return;
        if (!everReachable && Date.now() - startedAt > STARTUP_BUDGET_MS) {
          setPhase("failed");
          return; // never came up — stop and show the error
        }
        // Still booting (or a transient blip after it was reachable).
        setPhase((p) => (p === "blocked" ? "blocked" : "starting"));
        timer = window.setTimeout(tick, POLL_MS);
      }
    };

    // The host also emits assistant-timeout if its own 180s poll gives up; only
    // act on it if we never reached the backend.
    listen("assistant-timeout", () => {
      if (!cancelled && !everReachable) setPhase("failed");
    }).then((un) => {
      unlistenTimeout = un;
    });

    // Ask the host BEFORE polling: it knows at startup whether this installation
    // carries the Asistente at all, and whether the port was overridden
    // (MUNIGPT_PORT). On a scanner-only install the loop never starts, so the user is
    // not told "it failed to start" about something that was never shipped. If the
    // command itself fails we poll anyway — the old behaviour, and the safer default.
    void (async () => {
      try {
        const st = await invoke<AssistantStatus>("assistant_status");
        if (cancelled) return;
        if (st.apiBase) setApiBase(st.apiBase);
        if (st.installed === false) {
          setPhase("not-installed");
          return;
        }
      } catch {
        /* host command unavailable: fall through to polling the default base */
      }
      if (cancelled) return;
      startedAt = Date.now(); // the startup budget starts when polling does
      void tick();
    })();

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
      unlistenTimeout?.();
    };
  }, []);

  if (phase === "ready") {
    const webSearchEnabled = config?.webSearchEnabled ?? false;
    return (
      <div
        className={
          corpusNacional
            ? "asistente-view asistente-view--con-aviso"
            : "asistente-view"
        }
      >
        {corpusNacional && (
          <div className="asistente__corpus" role="status">
            Este equipo no tiene un corpus propio de la institución. El Asistente
            responde sobre la normativa nacional.
          </div>
        )}
        <Chat webSearchEnabled={webSearchEnabled} />
      </div>
    );
  }

  return (
    <div className="asistente-view asistente-view--status">
      <div className="asistente-status">
        {phase === "starting" && (
          <>
            <div className="asistente-status__spinner" aria-hidden="true" />
            Iniciando el asistente local. La primera carga puede tardar unos
            segundos en este equipo.
          </>
        )}
        {phase === "blocked" && (
          <>
            <div className="asistente-status__icon" aria-hidden="true">!</div>
            <p>{reason}</p>
            <ObtenerModelos />
            <p className="asistente-status__hint">
              Reintentando automáticamente; se activará en cuanto esté disponible.
            </p>
          </>
        )}
        {phase === "not-installed" && (
          <>
            <div className="asistente-status__icon" aria-hidden="true">i</div>
            <p>El Asistente no viene incluido en esta instalación.</p>
            <p className="asistente-status__hint">
              El escáner de cumplimiento funciona con normalidad. Para habilitar el
              Asistente hace falta el instalador que lo incluye; consulte con el área
              de TI de su institución.
            </p>
          </>
        )}
        {phase === "failed" && (
          <>
            <div className="asistente-status__icon" aria-hidden="true">!</div>
            <p>El Asistente no alcanzó a iniciarse en este equipo.</p>
            <p className="asistente-status__hint">
              El escáner de cumplimiento sigue funcionando con normalidad. Cierre y
              vuelva a abrir MuniANCI; si el problema persiste, avise al área de TI de
              su institución.
            </p>
          </>
        )}
      </div>
    </div>
  );
}
