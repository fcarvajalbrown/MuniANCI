// ObtenerModelos — las vías para conseguir el modelo que falta, y la elección de cuál.
//
// El GGUF de chat no cabe en el instalador (NSIS y WiX topan cerca de los 2 GB), así
// que llega por descarga reanudable o desde un paquete offline en un pendrive o una
// carpeta de red. La lógica vive en el backend (fetch_models.py) y las dos vías están
// cerradas por el SHA256 real del manifiesto: acá solo se disparan y se muestra el
// avance.
//
// Hay DOS modelos de chat y son alternativas, no una escalera: en un PC municipal de
// 8 GB corre el liviano y el grande no. El backend recomienda uno según la RAM del
// equipo, pero la elección es del usuario, y el motor usa el que haya en disco. Por eso
// cada modelo faltante trae su propio botón en vez de un único "descargar" que decide
// por él.
//
// El avance que informa el backend es tamaño en disco, no verificación: hashear 2,5 GB
// en cada consulta sería absurdo. Por eso dice "descargando" y no "verificado", y quien
// declara que un modelo sirve es /status.
//
// La carpeta del paquete se elige con un diálogo nativo que corre en Rust
// (assistant_pick_pack_dir): la capacidad de la ventana concede solo core:default, así
// que la API JS de diálogos está fuera de alcance a propósito.
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  elegirModelo,
  fetchModelsStatus,
  installModelsFromPack,
  startModelDownload,
  type ModelEntry,
  type ModelsStatus,
} from "../api";

const POLL_MS = 2_000;

/** 2497281120 -> "2,3 GB". Coma decimal, que es como se escribe en Chile. */
function tamano(bytes: number): string {
  const gb = bytes / 1024 ** 3;
  if (gb >= 1) return `${gb.toFixed(1).replace(".", ",")} GB`;
  return `${Math.round(bytes / 1024 ** 2)} MB`;
}

function porcentaje(bytes: number, total: number | null): number | null {
  if (!total || total <= 0) return null;
  return Math.min(100, Math.round((bytes / total) * 100));
}

export function ObtenerModelos() {
  const [estado, setEstado] = useState<ModelsStatus | null>(null);
  const [error, setError] = useState("");

  const refrescar = useCallback(async () => {
    try {
      setEstado(await fetchModelsStatus());
    } catch {
      /* el backend puede estar reiniciando; el sondeo del padre ya lo cubre */
    }
  }, []);

  useEffect(() => {
    let cancelado = false;
    let timer: number | undefined;

    const tick = async () => {
      if (cancelado) return;
      await refrescar();
      if (cancelado) return;
      timer = window.setTimeout(tick, POLL_MS);
    };
    void tick();

    return () => {
      cancelado = true;
      if (timer) clearTimeout(timer);
    };
  }, [refrescar]);

  const corriendo = estado?.tarea.estado === "corriendo";
  const enCurso = estado?.tarea.archivo ?? null;

  const descargar = async (archivo: string) => {
    setError("");
    try {
      setEstado(await startModelDownload(archivo));
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const desdePaquete = async (archivo: string) => {
    setError("");
    try {
      const dir = await invoke<string | null>("assistant_pick_pack_dir");
      if (!dir) return; // el usuario canceló
      setEstado(await installModelsFromPack(dir, archivo));
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const elegir = async (archivo: string) => {
    setError("");
    try {
      await elegirModelo(archivo);
      await invoke("asistente_reiniciar");
      await refrescar();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const faltantes = (estado?.modelos ?? []).filter((m) => !m.presente);
  const chatPresentes = (estado?.modelos ?? []).filter((m) => m.presente && m.esChat);

  if (estado && faltantes.length === 0 && chatPresentes.length < 2) return null;

  return (
    <div className="obtener-modelos">
      {faltantes.length === 0 && chatPresentes.length > 1 && (
        <ElegirModelo modelos={chatPresentes} onElegir={elegir} />
      )}
      {faltantes.map((m) => (
        <ModeloFaltante
          key={m.archivo}
          modelo={m}
          corriendo={corriendo}
          esElQueCorre={enCurso === m.archivo}
          onDescargar={() => descargar(m.archivo)}
          onPaquete={() => desdePaquete(m.archivo)}
        />
      ))}

      {corriendo && (
        <p className="asistente-status__hint">
          {estado?.tarea.accion === "paquete"
            ? "Copiando y verificando desde el paquete..."
            : "Descargando y verificando..."}
        </p>
      )}
      {estado?.tarea.estado === "error" && (
        <p className="obtener-modelos__error">
          No se pudo completar: {estado.tarea.error}
        </p>
      )}
      {error && <p className="obtener-modelos__error">{error}</p>}
      {estado && (
        <p className="asistente-status__hint">
          Los modelos se guardan en {estado.directorio}
        </p>
      )}
    </div>
  );
}

function ElegirModelo({
  modelos,
  onElegir,
}: {
  modelos: ModelEntry[];
  onElegir: (archivo: string) => void;
}) {
  return (
    <div className="elegir-modelo">
      <div className="elegir-modelo__titulo">Modelo del Asistente</div>
      <p className="elegir-modelo__ayuda">
        Los dos están en este equipo. El más grande responde mejor y el liviano responde
        antes; cambiar de modelo reinicia el Asistente.
      </p>
      {modelos.map((m) => (
        <div key={m.archivo} className="elegir-modelo__fila">
          <div>
            <div className="elegir-modelo__nombre">
              {m.nombre ?? m.archivo}
              {m.recomendado && (
                <span className="badge" style={{ marginLeft: "8px" }}>
                  recomendado para este equipo
                </span>
              )}
            </div>
            <div className="elegir-modelo__detalle">
              {m.bytesTotal ? tamano(m.bytesTotal) : tamano(m.bytes)} · {m.archivo}
            </div>
          </div>
          {m.enUso ? (
            <span className="badge badge--ok">En uso</span>
          ) : (
            <button className="btn btn--secondary" onClick={() => onElegir(m.archivo)}>
              Usar este
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

function ModeloFaltante({
  modelo,
  corriendo,
  esElQueCorre,
  onDescargar,
  onPaquete,
}: {
  modelo: ModelEntry;
  corriendo: boolean;
  esElQueCorre: boolean;
  onDescargar: () => void;
  onPaquete: () => void;
}) {
  const pct = porcentaje(modelo.bytes, modelo.bytesTotal);
  const total = modelo.bytesTotal ? tamano(modelo.bytesTotal) : "tamaño desconocido";

  return (
    <div className="obtener-modelos__modelo">
      <div className="obtener-modelos__nombre">
        {modelo.archivo}
        <span className="obtener-modelos__peso"> · {total}</span>
        {modelo.recomendado && (
          <span
            className="obtener-modelos__badge"
            title="Es el que corresponde a la memoria de este equipo. Puede elegir el otro igualmente."
          >
            recomendado para este equipo
          </span>
        )}
      </div>

      <div className="obtener-modelos__acciones">
        <button
          className={modelo.recomendado ? "btn btn--primary" : "btn btn--secondary"}
          onClick={onDescargar}
          disabled={corriendo || !modelo.descargable}
          title={
            modelo.descargable
              ? "Descarga con verificación SHA256; se reanuda si se corta"
              : "Este modelo no tiene un origen de descarga confirmado"
          }
        >
          Descargar
        </button>
        <button
          className="btn btn--secondary"
          onClick={onPaquete}
          disabled={corriendo}
          title="Instalar desde un pendrive o una carpeta de red, sin usar internet"
        >
          Desde un paquete offline...
        </button>
      </div>

      {esElQueCorre && (
        <>
          <div className="obtener-modelos__barra">
            <div
              className="obtener-modelos__avance"
              style={{ width: `${pct ?? 0}%` }}
            />
          </div>
          <div className="obtener-modelos__cifras">
            {tamano(modelo.bytes)}
            {modelo.bytesTotal ? ` de ${total}` : ""}
            {pct !== null ? ` (${pct}%)` : ""}
          </div>
        </>
      )}
    </div>
  );
}
