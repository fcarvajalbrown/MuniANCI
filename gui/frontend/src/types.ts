// TypeScript mirror of muniani-core types — must stay in sync with types.rs

export type Tier = "oiv" | "pse" | "unclassified";
export type Scope = "local" | "lan";
export type Severity = "critical" | "high" | "medium";
export type AppliesTo = "all" | "oiv_and_pse" | "oiv";
export type DriveKind = "fixed" | "removable" | "smb" | "nfs" | "web_dav" | "cloud_sync" | "unknown";
export type TlsCertIssue = "expired" | "self_signed" | "expired_and_self_signed";

export interface ScanProgress {
  pct: number;
  log: string;
}

export interface ScanMeta {
  institution_name: string;
  tier: Tier;
  scope: Scope;
}

/** Cómo se probó que el host está vivo, de evidencia más fuerte a más débil. */
export type DiscoveryMethod = "arp" | "icmp" | "tcp";

export interface Host {
  ip: string;
  hostname: string | null;
  mac: string | null;
  os_banner: string | null;
  /** Ausente cuando no hubo sondeo: el propio equipo, o un escaneo local. */
  discovered_by?: DiscoveryMethod;
  is_local: boolean;
}

export interface Drive {
  path: string;
  kind: DriveKind;
  total_bytes: number | null;
  free_bytes: number | null;
  encrypted: boolean | null;
  host_ip: string | null;
}

export interface Service {
  host_ip: string;
  port: number;
  banner: string | null;
  tls_version: string | null;
  tls_cert_issue: TlsCertIssue | null;
  anonymous_access: boolean;
}

export interface SoftwareEntry {
  name: string;
  version: string;
  host_ip: string;
  is_eol: boolean;
  max_cvss: number | null;
}

export interface OsInfo {
  host_ip: string;
  family: string;
  version: string;
  is_eol: boolean;
  firewall_active: boolean;
  backup_agent_running: boolean | null;
}

export interface AssetGraph {
  hosts: Host[];
  drives: Drive[];
  services: Service[];
  software: SoftwareEntry[];
  os_info: OsInfo[];
}

/** Si el control obliga hoy a esta institución, o solo se mide como madurez. */
export type Exigibilidad = "exigible" | "madurez_voluntaria";

/** Cómo clasifica la ley la infracción, cuando la clasifica. */
export type InfractionClass = "leve" | "grave" | "gravisima";

export interface Gap {
  control: string;
  finding: string;
  severity: Severity;
  legal_anchor: string;
  applies_to: AppliesTo;
  /** Exigible o madurez voluntaria. Sin esto la GUI no puede distinguir un
   *  incumplimiento legal de algo que la institución mide por su cuenta. */
  exigibilidad: Exigibilidad;
  /** `null` cuando la norma no fija escala sancionatoria para este control. */
  infraction_class: InfractionClass | null;
  /** Dominio de madurez. El prefijo `d7_` marca los del Decreto 7. */
  domain: string;
  evidence: string[];
  requires_csirt_report: boolean;
}

/** Marco normativo del que viene una brecha, deducido de su dominio. */
export function marcoDe(gap: Gap): "ley21663" | "decreto7" {
  return gap.domain.startsWith("d7_") ? "decreto7" : "ley21663";
}

// Como se movio un control entre la medicion anterior y esta. Espeja el enum
// `Estado` de core/src/historico.rs, que serializa en snake_case.
export type EstadoDeriva =
  | "nueva"
  | "persistente"
  | "resuelta"
  | "reaparecida"
  | "sin_verificar";

export interface ControlEnDeriva {
  control: string;
  estado: EstadoDeriva;
  /** Fecha en que se la vio cerrada, solo para una reaparecida. */
  resuelta_el: string | null;
}

export interface Deriva {
  desde: string | null;
  alcance_antes: string | null;
  alcance_ahora: string | null;
  /** Si este escaneo cubrio al menos lo que cubria el anterior. */
  cobertura_comparable: boolean;
  controles: ControlEnDeriva[];
}

export interface Delta {
  desde: string;
  puntaje: number;
  exigibles: number;
  criticas: number;
  cve_explotadas: number;
}

// Ley 21.180 — transformación digital. Otro cuerpo normativo: se informa, no se
// puntúa. Espeja `EstadoLey21180` en core/src/ley21180.rs.
export type GrupoLey21180 = "a" | "b" | "c";

export interface EstadoLey21180 {
  institucion: string;
  /** `null` cuando el nombre no figura en las listas del Art. 5° del DFL N°1. */
  grupo: GrupoLey21180 | null;
  anio: number;
  fases: string[];
  nota: string;
  procedencia: string;
}

export interface ScanResult {
  meta: ScanMeta;
  asset_graph: AssetGraph;
  gaps: Gap[];
  scanned_at: string;
  /** Ausente en un resultado generado antes de que existiera el bloque. */
  ley21180?: EstadoLey21180 | null;
  /** Cuanto se movieron los agregados. Lo rellena quien lleva el historico. */
  delta?: Delta | null;
  /** Que control se movio, y hacia donde. Ver `Deriva` en core. */
  deriva?: Deriva | null;
}

// UTM fine scale — Art. 40° Ley 21.663
// Source: Ley 21.663, Diario Oficial 08/04/2024, Art. 40°
//
// Va indexada por la **clasificación legal de la infracción**, que es como el Art. 40°
// construye la escala, y no por la severidad técnica del hallazgo. No es lo mismo: la
// severidad la asigna este producto como criterio operativo, y la clasificación la
// asigna la ley. Espeja `InfractionClass::max_utm` en core/src/types.rs.
export const UTM_FINES: Record<InfractionClass, Record<"oiv" | "pse", number>> = {
  gravisima: { oiv: 40000, pse: 20000 },
  grave:     { oiv: 20000, pse: 10000 },
  leve:      { oiv: 10000, pse: 5000  },
};

// UTM value in CLP — verify current value at SII (https://www.sii.cl)
export const UTM_CLP_APPROX = 66000;

export function utmToCLP(utm: number): string {
  return (utm * UTM_CLP_APPROX).toLocaleString("es-CL", {
    style: "currency",
    currency: "CLP",
    maximumFractionDigits: 0,
  });
}
// ---------------------------------------------------------------------------
// Monitoreo continuo (0.6.0)
// ---------------------------------------------------------------------------

// Lo que devuelve el comando `estado_monitoreo`. Es la red de seguridad del
// reescaneo programado: si una politica de grupo impidio crear la tarea, este
// aviso es lo unico que le recuerda a la municipalidad que su medicion envejecio.
export interface EstadoMonitoreo {
  ultimoEscaneo: string | null;
  dias: number | null;
  vencido: boolean;
  umbralDias: number;
  mediciones: number;
  tareaProgramada: boolean;
  advertencia: string;
}

// Lo que devuelve `exportar_evidencia`.
export interface EvidenciaExportada {
  ruta: string;
  archivos: number;
  bytes: number;
  oxum: string;
  manifiesto: string;
  instrucciones: string;
}

// ---------------------------------------------------------------------------
// Registro de riesgos (0.7.0)
// ---------------------------------------------------------------------------

// Espeja `Riesgo` en core/src/historico.rs, que el comando serializa en camelCase.
// Los estados salen del ciclo de vida del modelo POA&M de OSCAL.
export type EstadoRiesgo =
  | "abierto"
  | "investigando"
  | "cerrado"
  | "falso_positivo"
  | "aceptado";

export interface RiesgoUi {
  id: string;
  control: string;
  estado: EstadoRiesgo;
  responsable: string | null;
  plazo: string | null;
  nota: string | null;
  /** Cuándo pasó a un estado terminal. Lo administra core, no la interfaz. */
  cerradoEl: string | null;
  actualizado: string;
}
