// Simplified results view for non-technical municipal staff.
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ControlEnDeriva, Deriva, ScanResult, Gap, Severity } from "../types";
import { marcoDe } from "../types";
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
// Solo se muestra multa cuando la ley efectivamente la contempla para este control.
//
// Antes se calculaba desde `gap.severity`, que es criterio tecnico de este producto y
// no la escala del Art. 40. El resultado era que a una municipalidad se le mostraba una
// cifra en pesos por un control que no le es exigible y que no acarrea sancion alguna.
// Con el Decreto 7 la falla se volvia evidente: son diez controles cuyo decreto no fija
// escala sancionatoria, y todos habrian salido con multa.
function FineInfo({ gap, tier }: { gap: Gap; tier: "oiv" | "pse" }) {
  if (gap.exigibilidad !== "exigible" || gap.infraction_class === null) {
    return (
      <div className="worker-gap__fines">
        <span className="fine-chip" style={{ fontSize: "11px", color: "var(--text-muted)" }}>
          Sin multa asociada: {gap.exigibilidad === "exigible"
            ? "la norma no fija una escala sancionatoria para este control."
            : "no es exigible hoy a esta institución, se mide como madurez."}
        </span>
      </div>
    );
  }
  const utm = UTM_FINES[gap.infraction_class][tier];
  return (
    <div className="worker-gap__fines">
      <span className="fine-chip">
        Multa hasta: <strong>{utm.toLocaleString("es-CL")} UTM</strong>
      </span>
      <span className="fine-chip">
        ≈ <strong>{utmToCLP(utm)}</strong>
      </span>
      <span className="fine-chip" style={{ fontSize: "10px", color: "var(--text-muted)" }}>
        Infracción {INFRACCION_LABEL[gap.infraction_class]} · Art. 40° Ley 21.663 · valor UTM aprox. ${UTM_CLP_APPROX.toLocaleString("es-CL")} — verificar en SII
      </span>
    </div>
  );
}

const INFRACCION_LABEL: Record<NonNullable<Gap["infraction_class"]>, string> = {
  leve: "leve",
  grave: "grave",
  gravisima: "gravísima",
};

// Las fases del Art. 6° del DFL N°1, en las palabras del decreto. El backend manda
// el identificador (`cuatro`) y no la frase, asi que la traduccion vive aca.
const FASE_LABEL: Record<string, string> = {
  preparacion: "Preparación: identificar y describir las etapas de los procedimientos administrativos",
  uno:    "Fase 1: comunicaciones oficiales entre órganos en plataforma electrónica",
  dos:    "Fase 2: notificaciones por medios electrónicos",
  tres:   "Fase 3: ingreso de solicitudes y documentos por medios electrónicos",
  cuatro: "Fase 4: el procedimiento consta en un expediente electrónico",
  cinco:  "Fase 5: lo presentado en papel se digitaliza e ingresa al expediente",
  seis:   "Fase 6: aplicación del principio de interoperabilidad",
};

function GapCard({ gap, tier }: { gap: Gap; tier: "oiv" | "pse" }) {
  return (
    <div className={`worker-gap worker-gap--${gap.severity}`}>
      <div className="worker-gap__header">
        <span className={`pill pill--${gap.severity}`}>{SEVERITY_LABEL[gap.severity]}</span>
        {gap.exigibilidad === "madurez_voluntaria" && (
          <span
            className="badge"
            title="No es una obligación vigente para esta institución: se informa para que pueda medirse."
          >
            Madurez voluntaria
          </span>
        )}
        {marcoDe(gap) === "decreto7" && (
          <span
            className="badge"
            title="Norma Técnica de Seguridad de la Información (Decreto 7 de 2023), sobre las plataformas que sustentan procedimientos administrativos."
          >
            Decreto 7
          </span>
        )}
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

// Que cambio desde la medicion anterior, control por control.
//
// El orden no es alfabetico: primero lo que empeoro. Una reaparecida arriba de
// todo, porque dice que una correccion no se sostuvo, y eso es lo que alguien
// tiene que ir a mirar hoy.
function DerivaPanel({ deriva }: { deriva: Deriva }) {
  const de = (estado: ControlEnDeriva["estado"]) =>
    deriva.controles.filter((c) => c.estado === estado);

  const grupos = [
    { estado: "reaparecida" as const, titulo: "Se habían corregido y volvieron", clase: "critical" },
    { estado: "nueva" as const,        titulo: "Nuevas",                          clase: "high" },
    { estado: "resuelta" as const,     titulo: "Resueltas",                        clase: "ok" },
    { estado: "sin_verificar" as const, titulo: "Sin verificar en este escaneo",   clase: "medium" },
  ].filter((g) => de(g.estado).length > 0);

  const fecha = (f: string | null) => (f ? f.slice(0, 10) : "—");

  return (
    <div>
      <div className="section-title">
        Qué cambió desde la última evaluación ({fecha(deriva.desde)})
      </div>

      {/* Una cobertura menor no puede pasar inadvertida: es lo que separa
          "se corrigió" de "no se miró". */}
      {!deriva.cobertura_comparable && (
        <div className="deriva-aviso">
          Este escaneo cubrió menos que el anterior ({deriva.alcance_antes ?? "desconocido"} →{" "}
          {deriva.alcance_ahora ?? "desconocido"}). Los controles técnicos que faltan figuran
          como <strong>sin verificar</strong>, no como resueltos.
        </div>
      )}

      {grupos.length === 0 ? (
        <p className="state-panel__body" style={{ fontSize: "12px" }}>
          Sin cambios respecto de la medición anterior.
        </p>
      ) : (
        grupos.map((g) => (
          <div key={g.estado} className="deriva-grupo">
            <div className={`deriva-grupo__titulo deriva-grupo__titulo--${g.clase}`}>
              {g.titulo} ({de(g.estado).length})
            </div>
            <ul className="deriva-lista">
              {de(g.estado).map((c, i) => (
                <li key={i}>
                  {c.control}
                  {c.resuelta_el && (
                    <span className="deriva-lista__nota">
                      {" "}— estaba resuelta el {fecha(c.resuelta_el)}
                    </span>
                  )}
                </li>
              ))}
            </ul>
          </div>
        ))
      )}
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

      {/* Deriva: solo cuando hay con que comparar. En un primer escaneo no
          aparece, en vez de dejar un titulo vacio en pantalla. */}
      {result?.deriva?.desde && <DerivaPanel deriva={result.deriva} />}

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

      {/* Va aquí, pegado a las cifras, y no al final de la vista: es lo que hace el
          funcionario en cuanto ve los números, sin recorrer las 31 brechas primero. */}
      {result && <ExportarEjecutivo result={result} />}

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

      {/* Ley 21.180: otra norma, y por eso su propia tarjeta con su descargo.
          Sin el descargo, una seccion sobre transformacion digital dentro de un
          informe de la Ley 21.663 se lee como parte del mismo juicio. */}
      {result?.ley21180 && (
        <div className="card">
          <div className="section-title">Ley 21.180 — Transformación Digital del Estado</div>
          <p style={{ fontSize: "12px", color: "var(--text-muted)", lineHeight: 1.6 }}>
            Dato informativo de otra norma: no afecta el puntaje de cumplimiento ni la
            madurez de esta evaluación.
          </p>
          <p style={{ fontSize: "13px", lineHeight: 1.7 }}>
            {result.ley21180.grupo
              ? <>Grupo <strong>{result.ley21180.grupo.toUpperCase()}</strong> del Art. 5° del
                  DFL N°1, año {result.ley21180.anio}.</>
              : <>La institución no figura en las listas de municipalidades del Art. 5° del DFL N°1.</>}
          </p>
          {result.ley21180.fases.length > 0 && (
            <ul className="deriva-lista">
              {result.ley21180.fases.map((f, i) => (
                <li key={i}>{FASE_LABEL[f] ?? f}</li>
              ))}
            </ul>
          )}
          <p style={{ fontSize: "11px", color: "var(--text-muted)", lineHeight: 1.6 }}>
            {result.ley21180.procedencia}
          </p>
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

/// Exportación de la Vista Municipal: solo el informe ejecutivo.
///
/// Deliberadamente no ofrece el informe técnico ni el JSON del CSIRT. No es por
/// simplificar la pantalla: el técnico lleva IP, servicios y rutas de recursos
/// compartidos, y el propio `report_builder` dice que conviene tratarlo como
/// reservado. El ejecutivo es el documento que se le manda al alcalde, y esta es la
/// vista de quien lo manda. Lo demás sigue en la Vista Técnica, que es de TI.
function ExportarEjecutivo({ result }: { result: ScanResult }) {
  const [estado, setEstado] = useState<"idle" | "exportando" | "listo" | "error">("idle");
  const [mensaje, setMensaje] = useState<string | null>(null);

  async function exportar() {
    setEstado("exportando");
    setMensaje(null);
    try {
      const ruta = await invoke<string>("export_report", {
        result,
        format: "ejecutivo",
      });
      setEstado("listo");
      setMensaje(`Informe guardado en: ${ruta}`);
    } catch (e) {
      // Cancelar el diálogo tambien llega acá; se dice sin alarmar.
      const texto = String(e);
      setEstado(texto.includes("cancel") || texto.includes("Cancel") ? "idle" : "error");
      setMensaje(texto.includes("cancel") || texto.includes("Cancel") ? null : texto);
    }
  }

  return (
    <div className="export-bar">
      <span className="export-bar__label">Informe para la autoridad</span>
      <button
        className="btn btn--primary"
        disabled={estado === "exportando"}
        title="Resumen de una página: dónde estamos, qué arriesgamos y qué hay que autorizar"
        onClick={exportar}
      >
        {estado === "exportando" ? "Exportando..." : "Exportar informe ejecutivo (PDF)"}
      </button>
      {mensaje && (
        <span
          style={{
            fontSize: "11px",
            fontFamily: "var(--font-mono)",
            color: estado === "error" ? "var(--critical-text)" : "var(--ok-text)",
          }}
        >
          {mensaje}
        </span>
      )}
    </div>
  );
}