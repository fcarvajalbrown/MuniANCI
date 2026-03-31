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

export interface Host {
  ip: string;
  hostname: string | null;
  mac: string | null;
  os_banner: string | null;
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

export interface Gap {
  control: string;
  finding: string;
  severity: Severity;
  legal_anchor: string;
  applies_to: AppliesTo;
  evidence: string[];
  requires_csirt_report: boolean;
}

export interface ScanResult {
  meta: ScanMeta;
  asset_graph: AssetGraph;
  gaps: Gap[];
  scanned_at: string;
}

// UTM fine scale — Art. 40° Ley 21.663
// Source: Ley 21.663, Diario Oficial 08/04/2024, Art. 40°
export const UTM_FINES: Record<Severity, Record<"oiv" | "pse", number>> = {
  critical: { oiv: 40000, pse: 20000 },
  high:     { oiv: 20000, pse: 10000 },
  medium:   { oiv: 10000, pse: 5000  },
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