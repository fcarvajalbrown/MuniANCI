// Full technical gap dashboard for IT staff — terminal, evidence table, export controls.
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { EvidenciaExportada, ScanResult, Gap, Severity, SoftwareEntry, Service, OsInfo, RiesgoUi } from "../types";

interface Props {
  scanState:   "idle" | "scanning" | "done" | "error";
  progress:    number;
  logs:        string[];
  result:      ScanResult | null;
  error:       string | null;
  onStartScan: () => void;
}

const SEVERITY_LABEL: Record<Severity, string> = {
  critical: "Crítico",
  high:     "Alto",
  medium:   "Medio",
};

type ExportFormat = "pdf" | "json";
type ExportState  = "idle" | "exporting" | "done" | "error";

// ── Sub-components ───────────────────────────────────────────────────────────

function Terminal({ logs, scanning }: { logs: string[]; scanning: boolean }) {
  const bottomRef = useRef<HTMLDivElement>(null);
  useEffect(() => { bottomRef.current?.scrollIntoView({ behavior: "smooth" }); }, [logs]);

  return (
    <div className="terminal" aria-label="Terminal de escaneo">
      {logs.length === 0 && (
        <span className="terminal__line" style={{ color: "var(--text-muted)" }}>
          — En espera de inicio de escaneo —
        </span>
      )}
      {logs.map((line, i) => (
        <div
          key={i}
          className={`terminal__line ${
            i === logs.length - 1 && scanning ? "terminal__line--active" : "terminal__line--done"
          }`}
        >
          {line}
        </div>
      ))}
      <div ref={bottomRef} />
    </div>
  );
}

function GapRow({ gap }: { gap: Gap }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <>
      <tr
        onClick={() => setExpanded((v) => !v)}
        style={{ cursor: "pointer" }}
        title="Clic para expandir evidencia"
      >
        <td>
          <span className={`pill pill--${gap.severity}`}>
            {SEVERITY_LABEL[gap.severity]}
          </span>
        </td>
        <td>{gap.control}</td>
        <td>{gap.finding}</td>
        <td>
          {gap.exigibilidad === "exigible" ? (
            <span
              className="pill pill--critical"
              title="Obligación vigente hoy para esta institución. Su incumplimiento es auditable por la ANCI."
            >
              Exigible
            </span>
          ) : (
            <span
              className="badge"
              title="No es una obligación vigente para esta institución: se informa para que pueda medirse."
            >
              Madurez
            </span>
          )}
        </td>
        <td style={{ fontSize: "11px", color: "var(--text-muted)" }}>
          {gap.legal_anchor}
        </td>
        <td>
          {gap.requires_csirt_report && (
            <span className="badge badge--csirt" title="Art. 9° Ley 21.663 — notificar CSIRT ≤3h">
              CSIRT
            </span>
          )}
        </td>
      </tr>
      {expanded && (
        <tr>
          <td colSpan={6} style={{ background: "var(--bg-input)", padding: "var(--space-3) var(--space-5)" }}>
            <div className="section-title" style={{ marginBottom: "var(--space-2)" }}>Evidencia</div>
            {gap.evidence.map((e, i) => (
              <div key={i} className="evidence" style={{ fontFamily: "var(--font-mono)", fontSize: "11px", color: "var(--text-secondary)", lineHeight: 1.8 }}>
                {e}
              </div>
            ))}
          </td>
        </tr>
      )}
    </>
  );
}

function AssetSection({ result }: { result: ScanResult }) {
  const eolSw: SoftwareEntry[] = result.asset_graph.software.filter((s) => s.is_eol);
  const badServices: Service[] = result.asset_graph.services.filter(
    (s) => s.tls_cert_issue !== null || s.anonymous_access
  );
  const osInfo: OsInfo[] = result.asset_graph.os_info;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-5)" }}>

      {/* OS info */}
      <div>
        <div className="section-title">Sistema Operativo</div>
        <table className="gap-table">
          <thead>
            <tr>
              <th>Host</th><th>Familia</th><th>Versión</th><th>EOL</th><th>Firewall</th><th>Agente Backup</th>
            </tr>
          </thead>
          <tbody>
            {osInfo.map((o, i) => (
              <tr key={i}>
                <td className="evidence">{o.host_ip}</td>
                <td>{o.family}</td>
                <td className="evidence">{o.version}</td>
                <td>{o.is_eol ? <span className="pill pill--critical">EOL</span> : <span style={{ color: "var(--ok-text)", fontSize: "11px" }}>Vigente</span>}</td>
                <td>{o.firewall_active ? <span style={{ color: "var(--ok-text)", fontSize: "11px" }}>Activo</span> : <span className="pill pill--critical">Inactivo</span>}</td>
                <td>
                  {o.backup_agent_running === null
                    ? <span style={{ color: "var(--text-muted)", fontSize: "11px" }}>N/D (WMI)</span>
                    : o.backup_agent_running
                    ? <span style={{ color: "var(--ok-text)", fontSize: "11px" }}>Detectado</span>
                    : <span className="pill pill--high">No detectado</span>}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* EOL software */}
      {eolSw.length > 0 && (
        <div>
          <div className="section-title">Software en EOL ({eolSw.length})</div>
          <table className="gap-table">
            <thead>
              <tr><th>Nombre</th><th>Versión</th><th>Host</th></tr>
            </thead>
            <tbody>
              {eolSw.map((s, i) => (
                <tr key={i}>
                  <td>{s.name}</td>
                  <td className="evidence">{s.version}</td>
                  <td className="evidence">{s.host_ip}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Flagged services */}
      {badServices.length > 0 && (
        <div>
          <div className="section-title">Servicios con Observaciones ({badServices.length})</div>
          <table className="gap-table">
            <thead>
              <tr><th>Host</th><th>Puerto</th><th>TLS</th><th>Cert.</th><th>Acceso Anónimo</th></tr>
            </thead>
            <tbody>
              {badServices.map((s, i) => (
                <tr key={i}>
                  <td className="evidence">{s.host_ip}</td>
                  <td className="evidence">{s.port}</td>
                  <td className="evidence">{s.tls_version ?? "—"}</td>
                  <td>
                    {s.tls_cert_issue
                      ? <span className="pill pill--high">{s.tls_cert_issue}</span>
                      : <span style={{ color: "var(--ok-text)", fontSize: "11px" }}>OK</span>}
                  </td>
                  <td>
                    {s.anonymous_access
                      ? <span className="pill pill--critical">Sí</span>
                      : <span style={{ color: "var(--ok-text)", fontSize: "11px" }}>No</span>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function ExportBar({ result }: { result: ScanResult }) {
  const [state, setState]   = useState<ExportState>("idle");
  const [message, setMessage] = useState<string | null>(null);

  async function doExport(format: ExportFormat) {
    setState("exporting");
    setMessage(null);
    try {
      const path = await invoke<string>("export_report", { result, format });
      setState("done");
      setMessage(`Archivo guardado en: ${path}`);
    } catch (e) {
      setState("error");
      setMessage(String(e));
    }
  }

  async function exportarEvidencia() {
    setState("exporting");
    setMessage(null);
    try {
      const p = await invoke<EvidenciaExportada>("exportar_evidencia", { result });
      setState("done");
      // Se dice como verificarlo y con que: un sello que nadie sabe comprobar no
      // sirve de nada.
      setMessage(
        `Paquete en ${p.ruta} — ${p.archivos} archivos. ` +
        `Verifíquelo con certutil -hashfile contra ${p.manifiesto}; ver ${p.instrucciones}.`,
      );
    } catch (e) {
      setState("error");
      setMessage(String(e));
    }
  }

  return (
    <div className="export-bar">
      <span className="export-bar__label">Exportar Informe</span>
      <button
        className="btn btn--secondary"
        disabled={state === "exporting"}
        onClick={() => doExport("pdf")}
      >
        {state === "exporting" ? "Exportando..." : "Exportar PDF"}
      </button>
      <button
        className="btn btn--secondary"
        disabled={state === "exporting"}
        onClick={() => doExport("json")}
      >
        {state === "exporting" ? "Exportando..." : "Exportar JSON (CSIRT)"}
      </button>
      {/* El paquete es una carpeta, no un archivo: el manifiesto no vale nada
          separado de lo que sella. */}
      <button
        className="btn btn--primary"
        disabled={state === "exporting"}
        title="Carpeta fechada con los informes, el plan y un manifiesto SHA-256 verificable"
        onClick={exportarEvidencia}
      >
        {state === "exporting" ? "Generando..." : "Paquete de evidencia"}
      </button>
      {message && (
        <span
          style={{
            fontSize: "11px",
            fontFamily: "var(--font-mono)",
            color: state === "error" ? "var(--critical-text)" : "var(--ok-text)",
          }}
        >
          {message}
        </span>
      )}
    </div>
  );
}

// ── Main component ───────────────────────────────────────────────────────────

// Registro de riesgos: seguir cada hallazgo hasta cerrarlo.
//
// El plan de remediacion dice que hay que hacer; esto dice quien lo esta haciendo y
// como va. Sin la pantalla, el seguimiento existiria solo en la base de datos.
const ESTADOS: { valor: string; etiqueta: string; ayuda: string }[] = [
  { valor: "abierto",        etiqueta: "Abierto",         ayuda: "Sin trabajo declarado todavía." },
  { valor: "investigando",   etiqueta: "Investigando",    ayuda: "Se está averiguando si corresponde, o cómo corregirlo." },
  { valor: "cerrado",        etiqueta: "Corregido",       ayuda: "Corregido y verificado." },
  { valor: "falso_positivo", etiqueta: "Falso positivo",  ayuda: "Se revisó y el hallazgo no era real. No es lo mismo que corregido." },
  { valor: "aceptado",       etiqueta: "Riesgo aceptado", ayuda: "Se asume a sabiendas. No es cumplimiento: queda registrado con su justificación." },
];

function RegistroRiesgos({ result }: { result: ScanResult }) {
  const [filas, setFilas] = useState<RiesgoUi[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<RiesgoUi[]>("listar_riesgos")
      .then(setFilas)
      .catch((e) => setError(String(e)));
  }, [result.scanned_at]);

  const porControl = new Map(filas.map((r) => [r.control, r]));

  async function cambiar(gap: Gap, estado: string) {
    const previo = porControl.get(gap.control);
    try {
      const guardado = await invoke<RiesgoUi>("anotar_riesgo", {
        // El identificador no se manda: lo deriva core del nombre del control, para que
        // sea el mismo UUID que el POA&M emite en `risk/uuid`.
        control: gap.control,
        estado,
        responsable: previo?.responsable ?? null,
        plazo: previo?.plazo ?? null,
        nota: previo?.nota ?? null,
      });
      setFilas((f) => [...f.filter((x) => x.id !== guardado.id), guardado]);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  if (result.gaps.length === 0) return null;

  return (
    <div className="card">
      <div className="section-title">Seguimiento de Riesgos</div>
      <p style={{ fontSize: "12px", color: "var(--text-muted)", lineHeight: 1.6 }}>
        El estado se conserva entre escaneos y se emite en el POA&amp;M que entrega la
        municipalidad. Un riesgo aceptado no se informa como corregido.
      </p>
      {error && (
        <p style={{ fontSize: "12px", color: "var(--danger, #c0392b)" }}>{error}</p>
      )}
      <table className="gap-table">
        <tbody>
          {result.gaps.map((gap, i) => {
            const actual = porControl.get(gap.control)?.estado ?? "abierto";
            return (
              <tr key={i}>
                <td style={{ width: "55%" }}>{gap.control}</td>
                <td>
                  <select
                    value={actual}
                    onChange={(e) => cambiar(gap, e.target.value)}
                    title={ESTADOS.find((x) => x.valor === actual)?.ayuda}
                  >
                    {ESTADOS.map((o) => (
                      <option key={o.valor} value={o.valor}>{o.etiqueta}</option>
                    ))}
                  </select>
                </td>
                <td className="evidence" style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                  {porControl.get(gap.control)?.cerradoEl?.slice(0, 10) ?? ""}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export function ItTab({ scanState, progress, logs, result, error, onStartScan }: Props) {
  const scanning = scanState === "scanning";
  const [consolaOk, setConsolaOk] = useState(false);
  const [consolaAviso, setConsolaAviso] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("consola_disponible")
      .then(setConsolaOk)
      .catch(() => setConsolaOk(false));
  }, []);

  const abrirConsola = async () => {
    setConsolaAviso(null);
    try {
      const ruta = await invoke<string>("abrir_consola", { result });
      setConsolaAviso(result ? `Escaneo disponible en ${ruta}` : "Consola abierta.");
    } catch (e) {
      setConsolaAviso(String(e));
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-6)" }}>

      {/* Scan control + terminal — always visible */}
      <div className="card">
        <div className="section-title">Control de Escaneo</div>
        <div style={{ display: "flex", alignItems: "center", gap: "var(--space-4)", marginBottom: "var(--space-4)" }}>
          <button
            className="btn btn--primary"
            disabled={scanning}
            onClick={onStartScan}
          >
            {scanning ? "Escaneando..." : scanState === "done" ? "Nuevo Escaneo" : "Iniciar Escaneo"}
          </button>
          {consolaOk && (
            <button
              className="btn btn--secondary"
              onClick={() => void abrirConsola()}
              title="Abre una consola de apoyo en la carpeta del programa, con el escaneo actual a mano"
            >
              Consola de apoyo
            </button>
          )}
          {result && (
            <span style={{ fontSize: "11px", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
              Completado: {new Date(result.scanned_at).toLocaleString("es-CL")}
              {" · "}{result.meta.institution_name}
              {" · "}{result.meta.tier.toUpperCase()}
            </span>
          )}
        </div>

        {consolaAviso && (
          <div style={{ fontSize: "11px", color: "var(--text-muted)", fontFamily: "var(--font-mono)", marginBottom: "var(--space-3)" }}>
            {consolaAviso}
          </div>
        )}

        {/* Progress bar shown during scan */}
        {scanning && (
          <div className="progress-wrap" style={{ marginBottom: "var(--space-4)" }}>
            <div className="progress-label">
              <span>{logs[logs.length - 1] ?? "Iniciando..."}</span>
              <span>{progress}%</span>
            </div>
            <div className="progress-track">
              <div className="progress-fill" style={{ width: `${progress}%` }} />
            </div>
          </div>
        )}

        <Terminal logs={logs} scanning={scanning} />
      </div>

      {/* Error state */}
      {scanState === "error" && (
        <div className="state-panel__error" style={{ fontFamily: "var(--font-mono)", fontSize: "12px" }}>
          {error}
        </div>
      )}

      {/* Results */}
      {result && (
        <>
          {/* Export bar */}
          <ExportBar result={result} />

          {/* Gap table */}
          <div className="card">
            <div className="section-title">
              Brechas de Cumplimiento ({result.gaps.length})
            </div>
            {result.gaps.length === 0 ? (
              <p style={{ fontSize: "13px", color: "var(--ok-text)" }}>
                Sin brechas detectadas en los controles evaluados.
              </p>
            ) : (
              <table className="gap-table">
                <thead>
                  <tr>
                    <th>Severidad</th>
                    <th>Control</th>
                    <th>Hallazgo</th>
                    <th>Exigencia</th>
                    <th>Fundamento Legal</th>
                    <th>CSIRT</th>
                  </tr>
                </thead>
                <tbody>
                  {result.gaps.map((gap, i) => <GapRow key={i} gap={gap} />)}
                </tbody>
              </table>
            )}
          </div>

          {/* Asset graph detail */}
          <div className="card">
            <div className="section-title">Detalle de Activos</div>
            <AssetSection result={result} />
          </div>

          <RegistroRiesgos result={result} />

          {/* Scan metadata */}
          <div className="card card--elevated">
            <div className="section-title">Metadatos del Escaneo</div>
            <table className="gap-table">
              <tbody>
                <tr><td style={{ color: "var(--text-muted)", width: 180 }}>Institución</td><td className="evidence">{result.meta.institution_name}</td></tr>
                <tr><td style={{ color: "var(--text-muted)" }}>Clasificación</td><td className="evidence">{result.meta.tier.toUpperCase()}</td></tr>
                <tr><td style={{ color: "var(--text-muted)" }}>Alcance</td><td className="evidence">{result.meta.scope}</td></tr>
                <tr><td style={{ color: "var(--text-muted)" }}>Timestamp</td><td className="evidence">{result.scanned_at}</td></tr>
                <tr><td style={{ color: "var(--text-muted)" }}>Hosts descubiertos</td><td className="evidence">{result.asset_graph.hosts.length}</td></tr>
                <tr><td style={{ color: "var(--text-muted)" }}>Servicios analizados</td><td className="evidence">{result.asset_graph.services.length}</td></tr>
                <tr><td style={{ color: "var(--text-muted)" }}>Paquetes de software</td><td className="evidence">{result.asset_graph.software.length}</td></tr>
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}