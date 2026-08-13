import { useCallback, useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import type { ScanResult } from "../types";

interface Props {
  result: ScanResult | null;
}

export function Consola({ result }: Props) {
  const contenedor = useRef<HTMLDivElement | null>(null);
  const terminal   = useRef<Terminal | null>(null);
  const ajuste     = useRef<FitAddon | null>(null);
  const [abierta, setAbierta] = useState(false);
  const [error, setError]     = useState<string | null>(null);

  const redimensionar = useCallback(() => {
    if (!ajuste.current || !terminal.current) return;
    try {
      ajuste.current.fit();
    } catch {
      return;
    }
    void invoke("consola_redimensionar", {
      filas: terminal.current.rows,
      columnas: terminal.current.cols,
    }).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!abierta) return;
    const observador = new ResizeObserver(() => redimensionar());
    if (contenedor.current) observador.observe(contenedor.current);
    window.addEventListener("resize", redimensionar);
    return () => {
      observador.disconnect();
      window.removeEventListener("resize", redimensionar);
    };
  }, [abierta, redimensionar]);

  const abrir = async () => {
    if (abierta || !contenedor.current) return;
    setError(null);

    const term = new Terminal({
      fontFamily: "var(--font-mono), Consolas, monospace",
      fontSize: 13,
      cursorBlink: true,
      convertEol: false,
      theme: {
        background: "#0b1220",
        foreground: "#d7e2f4",
        cursor: "#7dd3fc",
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(contenedor.current);
    try {
      fit.fit();
    } catch {
      setError(null);
    }

    terminal.current = term;
    ajuste.current = fit;

    const salida = new Channel<string>();
    salida.onmessage = (texto) => term.write(texto);

    term.onData((datos) => {
      void invoke("consola_escribir", { datos }).catch(() => undefined);
    });

    try {
      await invoke("consola_iniciar", {
        salida,
        result,
        filas: term.rows,
        columnas: term.cols,
      });
      setAbierta(true);
    } catch (e) {
      setError(String(e));
      term.dispose();
      terminal.current = null;
      ajuste.current = null;
    }
  };

  const cerrar = async () => {
    await invoke("consola_cerrar").catch(() => undefined);
    terminal.current?.dispose();
    terminal.current = null;
    ajuste.current = null;
    setAbierta(false);
  };

  return (
    <div className="consola">
      <div className="consola__barra">
        <div>
          <div className="section-title" style={{ marginBottom: 0 }}>Consola de apoyo</div>
          <div className="consola__ayuda">
            Se abre en la carpeta del programa. El escaneo vigente queda a mano como{" "}
            <code>escaneo-actual.json</code>.
          </div>
        </div>
        <button
          className={`btn ${abierta ? "btn--secondary" : "btn--primary"}`}
          onClick={() => void (abierta ? cerrar() : abrir())}
        >
          {abierta ? "Cerrar consola" : "Abrir consola"}
        </button>
      </div>

      {error && <div className="consola__error">{error}</div>}

      <div className="consola__marco" ref={contenedor} />

      {!abierta && !error && (
        <div className="consola__vacio">
          La consola no está abierta.
        </div>
      )}
    </div>
  );
}
