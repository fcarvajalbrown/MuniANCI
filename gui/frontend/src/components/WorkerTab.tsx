// Simplified results view for non-technical municipal staff.
import type { ScanResult, Gap, Severity } from "../types";
import { UTM_FINES, UTM_CLP_APPROX, utmToCLP } from "../types";

interface Props {
  scanState: "idle" | "scanning" | "done" | "error";
  progress:  number;
  result:    ScanResult | null;
  error:     string | null;
  onStartScan: () => void;
}

const SEVERITY_LABEL: Record<Severity, string> = {
  critical: "Crítico",
  high:     "Alto",
  medium:   "Medio",
};

const SEVERITY_EXPLANATION: Record<Severity, string> = {
  critical: "Requiere atención inmediata. Expone a la institución a sanciones gravísimas y riesgo de incidente.",
  high:     "Debe ser corregido a la brevedad. Implica incumplimiento grave de la Ley 21.663.",
  medium:   "Incumplimiento leve. Debe ser abordado en el plan de mejora continua.",
};

// Source: Ley 21.663, D.O. 08/04/2024, Art. 40°
function FineInfo({ gap, tier }: { gap: Gap; tier: "oiv" | "pse" }) {
  const fines = UTM_FINES[gap.severity];
  const utm   = fines[tier];
  return (
    <div className="worker-gap__fines">
      <span className="fine-chip">
        Multa: <strong>{utm.toLocaleString("es-CL")} UTM</strong>
      </span>
      <span className="fine-chip">
        ≈ <strong>{utmToCLP(utm)}</strong>
      </span>
      <span className="fine-chip" style={{ fontSize: "10px", color: "var(--text-muted)" }}>
        Fuente: Art. 40° Ley 21.663 · valor UTM aprox. ${UTM_CLP_APPROX.toLocaleString("es-CL")} — verificar en SII
      </span>
    </div>
  );
}

function GapCard({ gap, tier }: { gap: Gap; tier: "oiv" | "pse" }) {
  return (
    <div className={`worker-gap worker-gap--${gap.severity}`}>
      <div className="worker-gap__header">
        <span className={`pill pill--${gap.severity}`}>{SEVERITY_LABEL[gap.severity]}</span>
        {gap.requires_csirt_report && (
          <span className="badge badge--csirt" title="Obliga reporte a CSIRT en ≤3 horas (Art. 9° Ley 21.663)">
            ⚠ Reporte CSIRT Obligatorio
          </span>
        )}
        <span className="worker-gap__control">{gap.control}</span>
      </div>

      <p className="worker-gap__finding">{gap.finding}</p>
      <p style={{ fontSize: "12px", color: "var(--text-secondary)", lineHeight: 1.6 }}>
        {SEVERITY_EXPLANATION[gap.severity]}
      </p>

      <p className="worker-gap__legal">
        Fundamento legal: {gap.legal_anchor}
        {/* Sources: Ley 21.663 D.O. 08/04/2024; ANCI IG N°1, N°3, N°4 dic 2025 */}
      </p>

      <FineInfo gap={gap} tier={tier} />
    </div>
  );
}

export function WorkerTab({ scanState, progress, result, error, onStartScan }: Props) {
  const tier = (result?.meta.tier ?? "pse") as "oiv" | "pse";

  const critical = result?.gaps.filter((g) => g.severity === "critical") ?? [];
  const high     = result?.gaps.filter((g) => g.severity === "high")     ?? [];
  const medium   = result?.gaps.filter((g) => g.severity === "medium")   ?? [];
  const allGaps  = [...critical, ...high, ...medium];

  // ── Idle ──────────────────────────────────────────────────────────────────
  if (scanState === "idle") {
    return (
      <div className="state-panel">
        <div className="state-panel__title">Evaluación de Cumplimiento</div>
        <p className="state-panel__body">
          Esta herramienta analiza el estado de ciberseguridad de la institución
          conforme a la <strong>Ley 21.663 — Marco de Ciberseguridad</strong> y las
          Instrucciones Generales de la ANCI. El proceso es completamente local
          y no transmite datos al exterior.
        </p>
        <p className="state-panel__body" style={{ fontSize: "11px", color: "var(--text-muted)" }}>
          Fuentes legales: Ley 21.663 D.O. 08/04/2024 · Ley 21.459 D.O. 20/06/2022 ·
          ANCI IG N°1 jun 2025 · ANCI IG N°3 y N°4 dic 2025
        </p>
        <button className="btn btn--primary btn--lg" onClick={onStartScan}>
          Iniciar Evaluación
        </button>
      </div>
    );
  }

  // ── Scanning ──────────────────────────────────────────────────────────────
  if (scanState === "scanning") {
    return (
      <div className="state-panel">
        <div className="state-panel__title">Análisis en Progreso</div>
        <div style={{ width: "100%", maxWidth: 480 }}>
          <div className="progress-wrap">
            <div className="progress-label">
              <span>Procesando...</span>
              <span>{progress}%</span>
            </div>
            <div className="progress-track">
              <div className="progress-fill" style={{ width: `${progress}%` }} />
            </div>
          </div>
        </div>
        <p className="state-panel__body" style={{ fontSize: "12px" }}>
          Por favor espere. El análisis puede tomar hasta 2 minutos
          dependiendo de la configuración de red.
        </p>
      </div>
    );
  }

  // ── Error ─────────────────────────────────────────────────────────────────
  if (scanState === "error") {
    return (
      <div className="state-panel">
        <div className="state-panel__title" style={{ color: "var(--critical-text)" }}>
          Error Durante el Análisis
        </div>
        <div className="state-panel__error">{error}</div>
        <button className="btn btn--secondary" onClick={onStartScan}>
          Reintentar
        </button>
      </div>
    );
  }

  // ── Done ──────────────────────────────────────────────────────────────────
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-6)" }}>

      {/* Summary stats */}
      <div>
        <div className="section-title">Resumen Ejecutivo</div>
        <div className="stats-row">
          <div className="stat-card stat-card--critical">
            <span className="stat-card__value">{critical.length}</span>
            <span className="stat-card__label">Brechas Críticas</span>
          </div>
          <div className="stat-card stat-card--high">
            <span className="stat-card__value">{high.length}</span>
            <span className="stat-card__label">Brechas Altas</span>
          </div>
          <div className="stat-card stat-card--medium">
            <span className="stat-card__value">{medium.length}</span>
            <span className="stat-card__label">Brechas Medias</span>
          </div>
          <div className={`stat-card ${allGaps.length === 0 ? "stat-card--ok" : "stat-card--critical"}`}>
            <span className="stat-card__value">{allGaps.length}</span>
            <span className="stat-card__label">Total Brechas</span>
          </div>
        </div>
      </div>

      {/* Legal context */}
      <div className="card">
        <div className="section-title">Marco Legal Aplicable</div>
        <p style={{ fontSize: "13px", color: "var(--text-secondary)", lineHeight: 1.8 }}>
          La institución está clasificada como{" "}
          <strong style={{ color: "var(--text-primary)" }}>
            {tier === "oiv"
              ? "Operador de Importancia Vital (OIV)"
              : "Prestador de Servicio Esencial (PSE)"}
          </strong>{" "}
          bajo la Ley 21.663 (D.O. 08/04/2024). Las brechas detectadas constituyen
          incumplimientos auditables por la Agencia Nacional de Ciberseguridad (ANCI).
          Las multas indicadas son referenciales conforme al Art. 40° y se expresan en
          Unidades Tributarias Mensuales (UTM) vigentes.
        </p>
        <p style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: "var(--space-3)" }}>
          Esta herramienta es de autoevaluación interna. La certificación de cumplimiento
          es competencia exclusiva de la ANCI. Valor UTM aproximado: $
          {UTM_CLP_APPROX.toLocaleString("es-CL")} CLP — verificar valor vigente en{" "}
          <strong>www.sii.cl</strong>.
        </p>
      </div>

      {/* Gap list */}
      {allGaps.length === 0 ? (
        <div className="state-panel">
          <div className="state-panel__title" style={{ color: "var(--ok-text)" }}>
            Sin Brechas Detectadas
          </div>
          <p className="state-panel__body">
            El análisis automatizado no identificó incumplimientos en los controles
            evaluados. Se recomienda mantener el plan de revisión continua conforme
            al Art. 8° lit. d) Ley 21.663.
          </p>
        </div>
      ) : (
        <div>
          <div className="section-title">
            Brechas Detectadas ({allGaps.length})
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-3)" }}>
            {allGaps.map((gap, i) => (
              <GapCard key={i} gap={gap} tier={tier} />
            ))}
          </div>
        </div>
      )}

      {/* CSIRT notice if any gap requires it */}
      {allGaps.some((g) => g.requires_csirt_report) && (
        <div className="card" style={{ borderColor: "var(--csirt)", background: "var(--csirt-dim)" }}>
          <div className="section-title" style={{ color: "var(--csirt-text)" }}>
            ⚠ Obligación de Reporte a CSIRT
          </div>
          <p style={{ fontSize: "13px", color: "var(--csirt-text)", lineHeight: 1.7 }}>
            Una o más brechas críticas detectadas superan el umbral de significancia
            del Art. 27° de la Ley 21.663 y obligan a notificar al CSIRT de Gobierno
            en un plazo máximo de <strong>3 horas</strong> desde la detección del
            incidente (Art. 9° Ley 21.663, D.O. 08/04/2024).
          </p>
        </div>
      )}

      <button
        className="btn btn--secondary"
        style={{ alignSelf: "flex-start" }}
        onClick={onStartScan}
      >
        Nueva Evaluación
      </button>
    </div>
  );
}