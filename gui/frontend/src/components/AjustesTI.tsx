import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigTI, EstadoTI, PapelTI, ResultadoGuardar } from "../types";

type Props = { onGuardado: (r: ResultadoGuardar) => void };

export function AjustesTI({ onGuardado }: Props) {
  const [abierto, setAbierto] = useState(false);
  const [estado, setEstado] = useState<EstadoTI | null>(null);
  const [config, setConfig] = useState<ConfigTI | null>(null);
  const [password, setPassword] = useState("");
  const [password2, setPassword2] = useState("");
  const [aviso, setAviso] = useState<string | null>(null);
  const contenedor = useRef<HTMLDivElement>(null);
  const sucio = useRef(false);

  const refrescarEstado = useCallback(async () => {
    const e = await invoke<EstadoTI>("ti_estado");
    setEstado(e);
    if (e.desbloqueado && !sucio.current) setConfig(await invoke<ConfigTI>("ti_leer"));
  }, []);

  const editarConfig = useCallback((c: ConfigTI) => {
    sucio.current = true;
    setConfig(c);
  }, []);

  const cerrarYOlvidar = useCallback(() => {
    sucio.current = false;
    setAbierto(false);
  }, []);

  useEffect(() => {
    if (abierto) {
      setAviso(null);
      refrescarEstado().catch((e) => setAviso(String(e)));
    }
  }, [abierto, refrescarEstado]);

  useEffect(() => {
    if (!abierto) return;
    const enfocables = () =>
      Array.from(
        contenedor.current?.querySelectorAll<HTMLElement>(
          ".ajustes__panel button, .ajustes__panel input, .ajustes__panel select"
        ) ?? []
      ).filter((el) => !el.hasAttribute("disabled"));

    enfocables()[0]?.focus();

    const alTeclear = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") {
        setAbierto(false);
        return;
      }
      if (ev.key !== "Tab") return;
      const items = enfocables();
      if (items.length === 0) return;
      const primero = items[0];
      const ultimo = items[items.length - 1];
      const activo = document.activeElement as HTMLElement | null;
      if (ev.shiftKey && (activo === primero || !contenedor.current?.contains(activo))) {
        ev.preventDefault();
        ultimo.focus();
      } else if (!ev.shiftKey && activo === ultimo) {
        ev.preventDefault();
        primero.focus();
      }
    };
    const alClicar = (ev: MouseEvent) => {
      if (contenedor.current && !contenedor.current.contains(ev.target as Node)) setAbierto(false);
    };
    document.addEventListener("keydown", alTeclear);
    document.addEventListener("mousedown", alClicar);
    return () => {
      document.removeEventListener("keydown", alTeclear);
      document.removeEventListener("mousedown", alClicar);
    };
  }, [abierto, estado?.desbloqueado]);

  const desbloquear = async () => {
    try {
      const ok = await invoke<boolean>("ti_desbloquear", { password });
      if (!ok) {
        setAviso("Contrasena incorrecta.");
        await refrescarEstado();
        return;
      }
      setPassword("");
      setAviso(null);
      await refrescarEstado();
    } catch (e) {
      setAviso(String(e));
      await refrescarEstado();
    }
  };

  const definirPassword = async () => {
    if (password !== password2) {
      setAviso("Las dos contrasenas no coinciden.");
      return;
    }
    try {
      await invoke("ti_definir_password", { password });
      setPassword("");
      setPassword2("");
      setAviso(null);
      await refrescarEstado();
    } catch (e) {
      setAviso(String(e));
    }
  };

  return (
    <div className="ajustes" ref={contenedor}>
      <button
        className="ajustes__cog"
        aria-label="Ajustes de TI"
        aria-expanded={abierto}
        onClick={() => setAbierto((v) => !v)}
      >
        &#9881;
      </button>

      {abierto && (
        <div className="ajustes__panel" role="dialog" aria-label="Ajustes de TI">
          <div className="ajustes__titulo">Ajustes de TI</div>

          {estado && !estado.conCandado && (
            <div className="ajustes__nota">Sin contrasena: build de desarrollo.</div>
          )}

          {aviso && (
            <div className="ajustes__error" role="alert">
              {aviso}
            </div>
          )}

          {estado && !estado.desbloqueado && !estado.passwordConfigurada && (
            <div className="ajustes__bloqueo">
              <p>Este equipo aun no tiene contrasena de TI. Defina una para continuar.</p>
              <input
                type="password"
                placeholder="Nueva contrasena"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
              <input
                type="password"
                placeholder="Repita la contrasena"
                value={password2}
                onChange={(e) => setPassword2(e.target.value)}
              />
              <button className="btn btn--primary" onClick={definirPassword}>
                Definir contrasena
              </button>
            </div>
          )}

          {estado && !estado.desbloqueado && estado.passwordConfigurada && (
            <div className="ajustes__bloqueo">
              <input
                type="password"
                placeholder="Contrasena de TI"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") desbloquear();
                }}
              />
              <button
                className="btn btn--primary"
                disabled={estado.esperaS > 0}
                onClick={desbloquear}
              >
                {estado.esperaS > 0 ? `Espere ${estado.esperaS} s` : "Desbloquear"}
              </button>
            </div>
          )}

          {estado?.desbloqueado && config && (
            <Secciones
              config={config}
              setConfig={editarConfig}
              estado={estado}
              setAviso={setAviso}
              onGuardado={onGuardado}
              cerrar={cerrarYOlvidar}
            />
          )}
        </div>
      )}
    </div>
  );
}

type SeccionesProps = {
  config: ConfigTI;
  setConfig: (c: ConfigTI) => void;
  estado: EstadoTI;
  setAviso: (s: string | null) => void;
  onGuardado: (r: ResultadoGuardar) => void;
  cerrar: () => void;
};

const SECCIONES = [
  { id: "identidad", titulo: "Identidad" },
  { id: "poam", titulo: "Plazos e historico" },
  { id: "red", titulo: "Red y monitoreo" },
  { id: "informe", titulo: "Informe" },
] as const;

const PAPELES: { valor: PapelTI; texto: string }[] = [
  { valor: "oficio", texto: "Oficio (21,6 x 33 cm)" },
  { valor: "carta", texto: "Carta (21,6 x 27,9 cm)" },
  { valor: "a4", texto: "A4 (21,0 x 29,7 cm)" },
];

const DIAS = ["lunes", "martes", "miercoles", "jueves", "viernes", "sabado", "domingo"];

function Secciones({ config, setConfig, estado, setAviso, onGuardado, cerrar }: SeccionesProps) {
  const [abierta, setAbierta] = useState<string | null>("identidad");
  const [guardando, setGuardando] = useState(false);

  const set = <K extends keyof ConfigTI>(clave: K, valor: ConfigTI[K]) =>
    setConfig({ ...config, [clave]: valor });

  const guardar = async () => {
    setGuardando(true);
    try {
      const r = await invoke<ResultadoGuardar>("ti_guardar", { nueva: config });
      if (r.requiereReinicioAsistente) {
        const seguir = window.confirm(
          "Cambiar la institucion reinicia el Asistente. Se pierde la conversacion abierta " +
            "y el backend puede tardar hasta tres minutos en volver a estar listo. Continuar?"
        );
        if (seguir) await invoke("asistente_reiniciar");
      }
      setAviso(null);
      onGuardado(r);
      cerrar();
    } catch (e) {
      setAviso(String(e));
    } finally {
      setGuardando(false);
    }
  };

  const restaurar = async (seccion: string) => {
    try {
      setConfig(await invoke<ConfigTI>("ti_restaurar_defectos", { seccion }));
    } catch (e) {
      setAviso(String(e));
    }
  };

  return (
    <>
      {SECCIONES.map((s) => (
        <div className="ajustes__seccion" key={s.id}>
          <button
            className="ajustes__cabecera"
            aria-expanded={abierta === s.id}
            onClick={() => setAbierta(abierta === s.id ? null : s.id)}
          >
            {s.titulo}
          </button>
          {abierta === s.id && (
            <div className="ajustes__campos">
              {s.id === "identidad" && (
                <>
                  <label>
                    Institucion
                    <input
                      type="text"
                      value={config.identidad.institucion ?? ""}
                      onChange={(e) =>
                        set("identidad", { ...config.identidad, institucion: e.target.value })
                      }
                    />
                  </label>
                  <label>
                    Clasificacion
                    <select
                      value={config.identidad.tier ?? "pse"}
                      onChange={(e) =>
                        set("identidad", { ...config.identidad, tier: e.target.value })
                      }
                    >
                      <option value="pse">Prestador de servicios esenciales</option>
                      <option value="oiv">Operador de importancia vital</option>
                      <option value="unclassified">Sin clasificar</option>
                    </select>
                  </label>
                  <p className="ajustes__ayuda">
                    Operador de importancia vital corresponde solo a quien la Agencia haya
                    calificado como tal por resolucion fundada. Sin clasificar apaga el deber de
                    reporte al CSIRT en todo el informe.
                  </p>
                </>
              )}

              {s.id === "poam" && (
                <>
                  <label>
                    Plazo brecha critica (dias)
                    <input
                      type="number"
                      min={1}
                      value={config.poam.plazo_dias_critica}
                      onChange={(e) =>
                        set("poam", { ...config.poam, plazo_dias_critica: Number(e.target.value) })
                      }
                    />
                  </label>
                  <label>
                    Plazo brecha alta (dias)
                    <input
                      type="number"
                      min={1}
                      value={config.poam.plazo_dias_alta}
                      onChange={(e) =>
                        set("poam", { ...config.poam, plazo_dias_alta: Number(e.target.value) })
                      }
                    />
                  </label>
                  <label>
                    Plazo brecha media (dias)
                    <input
                      type="number"
                      min={1}
                      value={config.poam.plazo_dias_media}
                      onChange={(e) =>
                        set("poam", { ...config.poam, plazo_dias_media: Number(e.target.value) })
                      }
                    />
                  </label>
                  <p className="ajustes__ayuda">
                    No son plazos legales. El unico plazo perentorio de la Ley 21.663 es el reporte
                    al CSIRT del Art. 9 (3 horas), que el informe trata aparte.
                  </p>
                  <label className="ajustes__check">
                    <input
                      type="checkbox"
                      checked={config.historico.habilitado}
                      onChange={(e) =>
                        set("historico", { ...config.historico, habilitado: e.target.checked })
                      }
                    />
                    Llevar historico de evaluaciones
                  </label>
                  <label className="ajustes__check">
                    <input
                      type="checkbox"
                      checked={config.historico.desglose_por_activo}
                      onChange={(e) =>
                        set("historico", {
                          ...config.historico,
                          desglose_por_activo: e.target.checked,
                        })
                      }
                    />
                    Guardar que activo arrastra cada brecha
                  </label>
                  <label>
                    Retencion (meses, 0 = nunca purgar)
                    <input
                      type="number"
                      min={0}
                      value={config.historico.retencion_meses}
                      onChange={(e) =>
                        set("historico", {
                          ...config.historico,
                          retencion_meses: Number(e.target.value),
                        })
                      }
                    />
                  </label>
                  <button className="ajustes__restaurar" onClick={() => restaurar("poam")}>
                    Restaurar plazos por defecto
                  </button>
                </>
              )}

              {s.id === "red" && (
                <>
                  <label className="ajustes__check">
                    <input
                      type="checkbox"
                      checked={config.red.arp}
                      onChange={(e) => set("red", { ...config.red, arp: e.target.checked })}
                    />
                    ARP
                  </label>
                  <label className="ajustes__check">
                    <input
                      type="checkbox"
                      checked={config.red.icmp}
                      onChange={(e) => set("red", { ...config.red, icmp: e.target.checked })}
                    />
                    ICMP
                  </label>
                  <label className="ajustes__check">
                    <input
                      type="checkbox"
                      checked={config.red.tcp}
                      onChange={(e) => set("red", { ...config.red, tcp: e.target.checked })}
                    />
                    TCP
                  </label>
                  <label>
                    Sondas ARP por segundo (0 = sin limite)
                    <input
                      type="number"
                      min={0}
                      value={config.red.arp_pps}
                      onChange={(e) => set("red", { ...config.red, arp_pps: Number(e.target.value) })}
                    />
                  </label>
                  <p className="ajustes__advertencia">
                    Si la red usa Dynamic ARP Inspection, subir este valor puede dejar el puerto en
                    err-disable, o sea este equipo se queda sin red hasta que el area de redes lo
                    rehabilite. Coordine el primer barrido de LAN completa con esa area.
                  </p>
                  <label>
                    Espera ICMP (ms)
                    <input
                      type="number"
                      min={1}
                      value={config.red.icmp_timeout_ms}
                      onChange={(e) =>
                        set("red", { ...config.red, icmp_timeout_ms: Number(e.target.value) })
                      }
                    />
                  </label>
                  <label>
                    Espera TCP (ms)
                    <input
                      type="number"
                      min={1}
                      value={config.red.tcp_timeout_ms}
                      onChange={(e) =>
                        set("red", { ...config.red, tcp_timeout_ms: Number(e.target.value) })
                      }
                    />
                  </label>
                  <label>
                    Hilos (0 = automatico)
                    <input
                      type="number"
                      min={0}
                      value={config.red.hilos}
                      onChange={(e) => set("red", { ...config.red, hilos: Number(e.target.value) })}
                    />
                  </label>
                  <label className="ajustes__check">
                    <input
                      type="checkbox"
                      checked={config.monitoreo.habilitado}
                      onChange={(e) =>
                        set("monitoreo", { ...config.monitoreo, habilitado: e.target.checked })
                      }
                    />
                    Reescaneo programado
                  </label>
                  <label>
                    Dia
                    <select
                      value={config.monitoreo.dia_semana}
                      onChange={(e) =>
                        set("monitoreo", { ...config.monitoreo, dia_semana: e.target.value })
                      }
                    >
                      {DIAS.map((d) => (
                        <option key={d} value={d}>
                          {d}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Hora
                    <input
                      type="time"
                      value={config.monitoreo.hora}
                      onChange={(e) => set("monitoreo", { ...config.monitoreo, hora: e.target.value })}
                    />
                  </label>
                  <label>
                    Avisar medicion vencida a los (dias)
                    <input
                      type="number"
                      min={1}
                      value={config.monitoreo.aviso_vencido_dias}
                      onChange={(e) =>
                        set("monitoreo", {
                          ...config.monitoreo,
                          aviso_vencido_dias: Number(e.target.value),
                        })
                      }
                    />
                  </label>
                  <button className="ajustes__restaurar" onClick={() => restaurar("red")}>
                    Restaurar red por defecto
                  </button>
                </>
              )}

              {s.id === "informe" && (
                <>
                  <label>
                    Papel del informe tecnico
                    <select
                      value={config.informe.tamano_papel_tecnico}
                      onChange={(e) =>
                        set("informe", {
                          ...config.informe,
                          tamano_papel_tecnico: e.target.value as PapelTI,
                        })
                      }
                    >
                      {PAPELES.map((p) => (
                        <option key={p.valor} value={p.valor}>
                          {p.texto}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Papel del informe ejecutivo
                    <select
                      value={config.informe.tamano_papel_ejecutivo}
                      onChange={(e) =>
                        set("informe", {
                          ...config.informe,
                          tamano_papel_ejecutivo: e.target.value as PapelTI,
                        })
                      }
                    >
                      {PAPELES.map((p) => (
                        <option key={p.valor} value={p.valor}>
                          {p.texto}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Color primario
                    <input
                      type="color"
                      value={config.informe.color_primario}
                      onChange={(e) =>
                        set("informe", { ...config.informe, color_primario: e.target.value })
                      }
                    />
                  </label>
                  <label>
                    Color de alerta
                    <input
                      type="color"
                      value={config.informe.color_alerta}
                      onChange={(e) =>
                        set("informe", { ...config.informe, color_alerta: e.target.value })
                      }
                    />
                  </label>
                  <button className="ajustes__restaurar" onClick={() => restaurar("informe")}>
                    Restaurar informe por defecto
                  </button>
                </>
              )}
            </div>
          )}
        </div>
      ))}

      <div className="ajustes__pie">
        <button className="btn btn--primary" disabled={guardando} onClick={guardar}>
          {guardando ? "Guardando..." : "Guardar"}
        </button>
        <button className="btn" onClick={cerrar}>
          Cancelar
        </button>
      </div>

      <div className="ajustes__extras">
        <button onClick={() => invoke("ti_abrir_archivo").catch((e) => setAviso(String(e)))}>
          Abrir el archivo de configuracion
        </button>
        <CambiarPassword setAviso={setAviso} />
        <p className="ajustes__origen">Configuracion leida de: {estado.origen}</p>
      </div>
    </>
  );
}

function CambiarPassword({ setAviso }: { setAviso: (s: string | null) => void }) {
  const [visible, setVisible] = useState(false);
  const [actual, setActual] = useState("");
  const [nueva, setNueva] = useState("");

  const cambiar = async () => {
    try {
      await invoke("ti_cambiar_password", { actual, nueva });
      setActual("");
      setNueva("");
      setVisible(false);
      setAviso(null);
    } catch (e) {
      setAviso(String(e));
    }
  };

  if (!visible) {
    return <button onClick={() => setVisible(true)}>Cambiar contrasena</button>;
  }
  return (
    <div className="ajustes__password">
      <input
        type="password"
        placeholder="Contrasena actual"
        value={actual}
        onChange={(e) => setActual(e.target.value)}
      />
      <input
        type="password"
        placeholder="Contrasena nueva"
        value={nueva}
        onChange={(e) => setNueva(e.target.value)}
      />
      <button className="btn btn--sm btn--primary" onClick={cambiar}>
        Guardar contrasena
      </button>
      <button className="btn btn--sm" onClick={() => setVisible(false)}>
        Cancelar
      </button>
    </div>
  );
}
