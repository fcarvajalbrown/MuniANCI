// Asistente tab — hosts the MuniGPT chat inside the MuniANCI window.
//
// The RAG backend runs as a Tauri sidecar (see gui/src/assistant.rs). It can take
// a while on first launch (the local model has to load), so this gates the chat
// behind a readiness check: it polls the `assistant_status` command until the
// backend reports ready, points the API client at the reported base URL, then
// loads per-municipality config and renders the chat. An `assistant-timeout`
// event from the host switches to an error message.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { fetchConfig, setApiBase, type AppConfig } from "../api";
import { Chat } from "./Chat";
import "../assistant.css";

interface AssistantStatus {
  running: boolean;
  ready: boolean;
  apiBase: string;
}

type Phase = "starting" | "ready" | "timeout";

export function AsistenteTab() {
  const [phase, setPhase] = useState<Phase>("starting");
  const [config, setConfig] = useState<AppConfig | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    let unlistenTimeout: (() => void) | undefined;

    const becomeReady = (base: string) => {
      if (cancelled) return;
      if (base) setApiBase(base);
      setPhase("ready");
      fetchConfig()
        .then((c) => !cancelled && setConfig(c))
        .catch(() => {
          /* config is optional; the chat still works with defaults */
        });
    };

    const poll = async () => {
      try {
        const st = await invoke<AssistantStatus>("assistant_status");
        if (cancelled) return;
        if (st.apiBase) setApiBase(st.apiBase);
        if (st.ready) {
          becomeReady(st.apiBase);
          return;
        }
      } catch {
        /* command not available yet; keep waiting */
      }
      if (!cancelled) timer = window.setTimeout(poll, 1500);
    };

    // The host emits assistant-timeout after ~180s if the backend never answers.
    listen("assistant-timeout", () => {
      if (!cancelled) setPhase((p) => (p === "ready" ? p : "timeout"));
    }).then((un) => {
      unlistenTimeout = un;
    });

    void poll();

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
      unlistenTimeout?.();
    };
  }, []);

  if (phase !== "ready") {
    return (
      <div className="asistente-view asistente-view--status">
        <div className="asistente-status">
          {phase === "starting" ? (
            <>
              <div className="asistente-status__spinner" aria-hidden="true" />
              Iniciando el asistente local. La primera carga del modelo puede tardar
              varios minutos en este equipo.
            </>
          ) : (
            <>
              El asistente no respondió a tiempo. Verifica que los modelos estén
              instalados y que el backend pueda iniciarse.
            </>
          )}
        </div>
      </div>
    );
  }

  const webSearchEnabled = config?.webSearchEnabled ?? false;
  return (
    <div className="asistente-view">
      <Chat webSearchEnabled={webSearchEnabled} />
    </div>
  );
}
