// Asistente tab — hosts the MuniGPT chat inside the MuniANCI window.
//
// The RAG backend runs as a Tauri sidecar (see gui/src/assistant.rs). Readiness
// is resolved from the backend's own /status payload so the UI never spins
// indefinitely:
//
//   - reachable + ready         -> render the chat
//   - reachable + NOT ready     -> show the concrete blocker at once (which model
//                                  file is missing, or the missing engine binary),
//                                  and keep polling so it self-heals if the file
//                                  is added — no endless spinner
//   - not reachable within the
//     startup budget            -> show a "backend failed to start" error
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
import "../assistant.css";

interface AssistantStatus {
  running: boolean;
  ready: boolean;
  apiBase: string;
}

type Phase = "starting" | "ready" | "blocked" | "failed";

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

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    let unlistenTimeout: (() => void) | undefined;
    let everReachable = false;
    const startedAt = Date.now();

    // Resolve the API base from the sidecar (honors MUNIGPT_PORT). Best-effort:
    // the client already defaults to 127.0.0.1:8000.
    invoke<AssistantStatus>("assistant_status")
      .then((st) => {
        if (!cancelled && st.apiBase) setApiBase(st.apiBase);
      })
      .catch(() => {});

    const tick = async () => {
      if (cancelled) return;
      try {
        const s = await fetchStatus();
        everReachable = true;
        if (cancelled) return;
        if (s.ready) {
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

    void tick();

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
      unlistenTimeout?.();
    };
  }, []);

  if (phase === "ready") {
    const webSearchEnabled = config?.webSearchEnabled ?? false;
    return (
      <div className="asistente-view">
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
            <p className="asistente-status__hint">
              Reintentando automáticamente; se activará en cuanto esté disponible.
            </p>
          </>
        )}
        {phase === "failed" && (
          <>
            <div className="asistente-status__icon" aria-hidden="true">!</div>
            <p>
              No se pudo iniciar el backend del asistente. Verifica que Python y
              sus dependencias estén instalados y que el puerto no esté ocupado.
            </p>
          </>
        )}
      </div>
    </div>
  );
}
