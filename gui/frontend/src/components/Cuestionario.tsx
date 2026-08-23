import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PreguntaCuestionario } from "../types";

type Props = { onCerrar: () => void; onGuardado: () => void };

type Borrador = Record<string, { cumple: boolean | null; nota: string }>;

function desdeBackend(preguntas: PreguntaCuestionario[]): Borrador {
  const b: Borrador = {};
  for (const p of preguntas) {
    b[p.clave] = { cumple: p.respondida ? p.cumple : null, nota: p.nota ?? "" };
  }
  return b;
}

export function Cuestionario({ onCerrar, onGuardado }: Props) {
  const [preguntas, setPreguntas] = useState<PreguntaCuestionario[] | null>(null);
  const [borrador, setBorrador] = useState<Borrador>({});
  const [aviso, setAviso] = useState<string | null>(null);
  const [guardando, setGuardando] = useState(false);

  const cargar = useCallback(async () => {
    try {
      const ps = await invoke<PreguntaCuestionario[]>("cuestionario_leer");
      setPreguntas(ps);
      setBorrador(desdeBackend(ps));
    } catch (e) {
      setAviso(String(e));
    }
  }, []);

  useEffect(() => {
    cargar();
  }, [cargar]);

  const responder = (clave: string, cumple: boolean) =>
    setBorrador((b) => ({ ...b, [clave]: { ...b[clave], cumple } }));

  const anotar = (clave: string, nota: string) =>
    setBorrador((b) => ({ ...b, [clave]: { ...b[clave], nota } }));

  const guardar = async () => {
    setGuardando(true);
    setAviso(null);
    try {
      const respuestas = Object.entries(borrador)
        .filter(([, v]) => v.cumple !== null)
        .map(([clave, v]) => ({ clave, cumple: v.cumple as boolean, nota: v.nota || null }));
      await invoke<string>("cuestionario_guardar", { respuestas });
      onGuardado();
      onCerrar();
    } catch (e) {
      setAviso(String(e));
    } finally {
      setGuardando(false);
    }
  };

  if (!preguntas) {
    return (
      <div className="state-panel">
        <div className="state-panel__title">Cuestionario</div>
        <p className="state-panel__body">{aviso ?? "Cargando preguntas..."}</p>
        <button className="btn btn--secondary" onClick={onCerrar}>
          Volver
        </button>
      </div>
    );
  }

  const sinResponder = Object.values(borrador).filter((v) => v.cumple === null).length;
  const dominios = [...new Set(preguntas.map((p) => p.dominio))];

  return (
    <div className="cuestionario">
      <div className="cuestionario__encabezado">
        <div>
          <div className="section-title">Cuestionario de cumplimiento</div>
          <p className="cuestionario__intro">
            Son los controles que ningún escaneo puede comprobar por sí solo: hay que
            declararlos. Una pregunta sin responder se informa como brecha, porque no se
            demostró cumplimiento, así que conviene contestarlas todas antes de evaluar.
          </p>
        </div>
        <div className="cuestionario__contador">
          {sinResponder === 0
            ? "Todas respondidas"
            : `${sinResponder} sin responder`}
        </div>
      </div>

      {aviso && <div className="aviso-historico">{aviso}</div>}

      {dominios.map((dominio) => (
        <div key={dominio} className="cuestionario__dominio">
          <div className="cuestionario__dominio-titulo">{dominio}</div>

          {preguntas
            .filter((p) => p.dominio === dominio)
            .map((p) => {
              const estado = borrador[p.clave];
              return (
                <div key={p.clave} className="cuestionario__item">
                  <div className="cuestionario__pregunta">
                    <span>{p.texto}</span>
                    <span
                      className={`pill ${p.exigible ? "pill--critical" : "pill--medium"}`}
                    >
                      {p.exigible ? "Exigible" : "Madurez voluntaria"}
                    </span>
                  </div>

                  <div className="cuestionario__meta">
                    {p.anclajeLegal} · severidad si no se cumple: {p.severidad}
                  </div>

                  <div className="cuestionario__opciones">
                    <label>
                      <input
                        type="radio"
                        name={p.clave}
                        checked={estado?.cumple === true}
                        onChange={() => responder(p.clave, true)}
                      />
                      Sí, se cumple
                    </label>
                    <label>
                      <input
                        type="radio"
                        name={p.clave}
                        checked={estado?.cumple === false}
                        onChange={() => responder(p.clave, false)}
                      />
                      No se cumple
                    </label>
                    {estado?.cumple === null && (
                      <span className="cuestionario__pendiente">Sin responder</span>
                    )}
                  </div>

                  <input
                    type="text"
                    className="cuestionario__nota"
                    placeholder={`Evidencia (opcional). Ejemplo: ${p.ejemploEvidencia}`}
                    value={estado?.nota ?? ""}
                    onChange={(e) => anotar(p.clave, e.target.value)}
                  />
                </div>
              );
            })}
        </div>
      ))}

      <div className="cuestionario__acciones">
        <button className="btn btn--primary" onClick={guardar} disabled={guardando}>
          {guardando ? "Guardando..." : "Guardar respuestas"}
        </button>
        <button className="btn btn--secondary" onClick={onCerrar} disabled={guardando}>
          Cancelar
        </button>
      </div>
    </div>
  );
}
