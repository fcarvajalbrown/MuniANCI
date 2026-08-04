// Root component — owns scan state, streams progress, routes between tabs.
import { useState, useRef, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import type { EstadoMonitoreo, ScanResult, ScanProgress } from "./types";
import { WorkerTab } from "./components/WorkerTab";
import { ItTab } from "./components/ItTab";
import { AsistenteTab } from "./components/AsistenteTab";
import { AjustesTI } from "./components/AjustesTI";
import "./app.css";

type Tab = "worker" | "it" | "asistente";
type ScanState = "idle" | "scanning" | "done" | "error";
type Branding = { institution: string; tier: string };

export default function App() {
  const [tab, setTab]           = useState<Tab>("worker");
  const [scanState, setScanState] = useState<ScanState>("idle");
  const [progress, setProgress] = useState(0);
  const [logs, setLogs]         = useState<string[]>([]);
  const [result, setResult]     = useState<ScanResult | null>(null);
  const [error, setError]       = useState<string | null>(null);
  const [branding, setBranding] = useState<Branding | null>(null);
  const [monitoreo, setMonitoreo] = useState<EstadoMonitoreo | null>(null);
  const [configVencida, setConfigVencida] = useState(false);
  const scanningRef             = useRef(false);

  const revisarBranding = useCallback(() => {
    invoke<Branding>("app_branding").then(setBranding).catch(() => {});
  }, []);

  useEffect(() => {
    revisarBranding();
  }, [revisarBranding]);

  // Cuanto lleva sin renovarse la medicion. Es la red de seguridad del reescaneo
  // programado: si una politica de grupo impidio crear la tarea, este aviso es lo
  // unico que le recuerda a la municipalidad que su medicion envejecio.
  const revisarMonitoreo = useCallback(() => {
    invoke<EstadoMonitoreo>("estado_monitoreo").then(setMonitoreo).catch(() => {});
  }, []);
  useEffect(revisarMonitoreo, [revisarMonitoreo]);

  const startScan = useCallback(async () => {
    if (scanningRef.current) return;
    scanningRef.current = true;
    setScanState("scanning");
    setProgress(0);
    setLogs([]);
    setResult(null);
    setError(null);
    setConfigVencida(false);

    const channel = new Channel<ScanProgress>();
    channel.onmessage = (msg) => {
      setProgress(msg.pct);
      setLogs((prev) => [...prev, `[${String(msg.pct).padStart(3, " ")}%] ${msg.log}`]);
    };

    try {
      const scanResult = await invoke<ScanResult>("start_scan", { onProgress: channel });
      revisarMonitoreo();
      setResult(scanResult);
      setScanState("done");
    } catch (e) {
      setError(String(e));
      setScanState("error");
    } finally {
      scanningRef.current = false;
    }
  }, []);

  const criticalCount = result?.gaps.filter((g) => g.severity === "critical").length ?? 0;
  const highCount     = result?.gaps.filter((g) => g.severity === "high").length ?? 0;
  const mediumCount   = result?.gaps.filter((g) => g.severity === "medium").length ?? 0;

  return (
    <div className="app">
      {monitoreo?.vencido && (
        <div className="aviso-vencido" role="status">
          <strong>La medicion esta vencida.</strong>{" "}
          El ultimo escaneo fue hace {monitoreo.dias} dias; el limite configurado es{" "}
          {monitoreo.umbralDias}.{" "}
          {monitoreo.tareaProgramada
            ? "Hay un reescaneo programado, pero no llego a correr."
            : "No hay un reescaneo programado en este equipo."}{" "}
          <button className="btn btn--primary btn--sm" onClick={startScan} disabled={scanState === "scanning"}>
            Escanear ahora
          </button>
        </div>
      )}
      {configVencida && (
        <div className="aviso-config" role="status">
          <strong>La configuracion cambio despues de este escaneo.</strong>{" "}
          El resultado en pantalla ya no corresponde a los ajustes vigentes.{" "}
          <button
            className="btn btn--primary btn--sm"
            onClick={startScan}
            disabled={scanState === "scanning"}
          >
            Escanear de nuevo
          </button>
        </div>
      )}
      <header className="app-header">
        <div className="app-header__brand">
          {/* Logo placeholder — to be defined */}
          <div className="app-header__logo-placeholder" aria-hidden="true">▣</div>
          <div className="app-header__title-block">
            <span className="app-header__title">{branding?.institution ?? "MuniANCI"}</span>
            <span className="app-header__subtitle">
              MuniANCI · Escáner de Cumplimiento — Ley 21.663
            </span>
          </div>
        </div>

        {result && (
          <div className="app-header__badges">
            {criticalCount > 0 && (
              <span className="badge badge--critical">{criticalCount} Crítico{criticalCount > 1 ? "s" : ""}</span>
            )}
            {highCount > 0 && (
              <span className="badge badge--high">{highCount} Alto{highCount > 1 ? "s" : ""}</span>
            )}
            {mediumCount > 0 && (
              <span className="badge badge--medium">{mediumCount} Medio{mediumCount > 1 ? "s" : ""}</span>
            )}
            {criticalCount === 0 && highCount === 0 && mediumCount === 0 && (
              <span className="badge badge--ok">Sin Brechas Detectadas</span>
            )}
          </div>
        )}

        <nav className="app-tabs" role="tablist">
          <button
            role="tab"
            aria-selected={tab === "worker"}
            className={`app-tab ${tab === "worker" ? "app-tab--active" : ""}`}
            onClick={() => setTab("worker")}
          >
            Vista Municipal
          </button>
          <button
            role="tab"
            aria-selected={tab === "it"}
            className={`app-tab ${tab === "it" ? "app-tab--active" : ""}`}
            onClick={() => setTab("it")}
          >
            Vista Técnica (TI)
          </button>
          <button
            role="tab"
            aria-selected={tab === "asistente"}
            className={`app-tab ${tab === "asistente" ? "app-tab--active" : ""}`}
            onClick={() => setTab("asistente")}
          >
            Asistente
          </button>
        </nav>

        <AjustesTI
          onGuardado={(r) => {
            if (r.afectaInforme && result) setConfigVencida(true);
            revisarBranding();
            revisarMonitoreo();
          }}
        />
      </header>

      <main className="app-main">
        {tab === "worker" && (
          <WorkerTab
            scanState={scanState}
            progress={progress}
            result={result}
            error={error}
            onStartScan={startScan}
          />
        )}
        {tab === "it" && (
          <ItTab
            scanState={scanState}
            progress={progress}
            logs={logs}
            result={result}
            error={error}
            onStartScan={startScan}
          />
        )}
        {/* Kept mounted (hidden when inactive) so the backend warms up in the
            background and chat history survives tab switches. */}
        <div
          className="app-main__pane"
          style={{
            display: tab === "asistente" ? "flex" : "none",
            flex: 1,
            minHeight: 0,
          }}
        >
          <AsistenteTab />
        </div>
      </main>

      <footer className="app-footer">
        <span>Felipe Carvajal Brown</span>
        <span className="app-footer__sep">·</span>
        <span>Clasificación: RESERVADO</span>
        <span className="app-footer__sep">·</span>
        <span>Herramienta de autoevaluación — no reemplaza certificación ANCI</span>
      </footer>
    </div>
  );
}