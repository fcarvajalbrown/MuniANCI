// ObtenerModelos — las dos vías para conseguir el modelo de chat que falta.
//
// El GGUF de chat pesa entre 1,3 y 2,5 GB y no cabe en el instalador (NSIS y WiX
// topan cerca de los 2 GB), así que llega por descarga reanudable o desde un paquete
// offline en un pendrive o una carpeta de red. La lógica vive en el backend
// (fetch_models.py) y ambas vías están cerradas por el SHA256 real del manifiesto:
// aquí solo se disparan y se muestra el avance.
//
// El avance que informa el backend es tamaño en disco, no verificación: hashear 2,5 GB
// en cada consulta sería absurdo. Por eso esta pantalla dice "descargando" y no
// "verificado", y quien declara que un modelo sirve es /status, no este componente.
//
// La carpeta del paquete se elige con un diálogo nativo que corre en Rust
// (assistant_pick_pack_dir): la capacidad de la ventana concede solo core:default, así
// que la API JS de diálogos está fuera de alcance a propósito.
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  fetchModelsStatus,
  installModelsFromPack,
  startModelDownload,
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

  // Mientras hay un trabajo corriendo se consulta cada dos segundos; en reposo
  // alcanza con una consulta, porque nada cambia sin que el usuario lo pida.
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

  const descargar = async () => {
    setError("");
    try {
      setEstado(await startModelDownload());
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const desdePaquete = async () => {
    setError("");
    try {
      const dir = await invoke<string | null>("assistant_pick_pack_dir");
      if (!dir) return; // el usuario canceló
      setEstado(await installModelsFromPack(dir));
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const faltantes = (estado?.modelos ?? []).filter((m) => !m.presente);
  const hayDescargable = faltantes.some((m) => m.descargable);

  return (
    <div className="obtener-modelos">
      <div className="obtener-modelos__acciones">
        <button
          className="btn btn--primary"
          onClick={descargar}
          disabled={corriendo || !hayDescargable}
          title={
            hayDescargable
              ? "Descarga con verificación SHA256; se reanuda si se corta"
              : "Ningún modelo faltante tiene un origen de descarga confirmado"
          }
        >
          Descargar ahora
        </button>
        <button
          className="btn btn--secondary"
          onClick={desdePaquete}
          disabled={corriendo}
          title="Instalar desde un pendrive o una carpeta de red, sin usar internet"
        >
          Usar un paquete offline...
        </button>
      </div>

      {faltantes.map((m) => {
        const pct = porcentaje(m.bytes, m.bytesTotal);
        return (
          <div className="obtener-modelos__modelo" key={m.archivo}>
            <div className="obtener-modelos__nombre">{m.archivo}</div>
            <div className="obtener-modelos__barra">
              <div
                className="obtener-modelos__avance"
                style={{ width: `${pct ?? 0}%` }}
              />
            </div>
            <div className="obtener-modelos__cifras">
              {tamano(m.bytes)}
              {m.bytesTotal ? ` de ${tamano(m.bytesTotal)}` : ""}
              {pct !== null ? ` (${pct}%)` : ""}
            </div>
          </div>
        );
      })}

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
