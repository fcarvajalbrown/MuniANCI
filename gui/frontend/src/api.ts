// api.ts — typed client for the MuniGPT (Asistente) backend.
//
// The chat endpoint streams Server-Sent Events over a POST body, so we cannot use
// the browser EventSource (GET-only). Instead we read the fetch ReadableStream and
// parse the `data: {...}\n\n` frames ourselves.
//
// Inside the Tauri host the frontend is served from a custom protocol, so the
// backend must be addressed absolutely. The sidecar listens on 127.0.0.1:8000 by
// default; the exact base comes from the `assistant_status` command (which honors
// the MUNIGPT_PORT override) and is set here via setApiBase().

let API_BASE = "http://127.0.0.1:8000";

/** Point the client at the backend base URL reported by the Tauri sidecar. */
export function setApiBase(base: string): void {
  if (base) API_BASE = base;
}

function apiUrl(path: string): string {
  return API_BASE + path;
}

export interface Citation {
  source: string;
  chunk_index: number;
}

export interface ChatMessage {
  role: "user" | "assistant" | "system";
  content: string;
}

export interface LicenseStatus {
  valid: boolean;
  state: "valid" | "expired" | "invalid" | "missing";
  reason: string;
  municipio?: string | null;
  issuedTo?: string | null;
  expiresAt?: string | null;
}

export interface AppConfig {
  municipio: string;
  logo: string;
  webSearchEnabled: boolean;
  licenseStatus?: LicenseStatus;
  [key: string]: unknown;
}

export interface SearchResult {
  title: string;
  url: string;
  snippet: string;
}

// A fixed category chip offered when a query is too vague to retrieve against
// (e.g. "cómo pagar su parte?"). Detection is a deterministic backend keyword
// classifier, not the LLM — see main.py `_is_ambiguous`.
export interface DisambiguationCategory {
  id: string;
  label: string;
}

export interface Disambiguation {
  message: string;
  categories: DisambiguationCategory[];
  pendingMessage: string;
}

export interface StreamHandlers {
  onCitations?: (citations: Citation[]) => void;
  onDisambiguate?: (info: Disambiguation) => void;
  onToken?: (token: string) => void;
  onError?: (message: string) => void;
  onDone?: () => void;
  signal?: AbortSignal;
}

export async function fetchConfig(): Promise<AppConfig> {
  const res = await fetch(apiUrl("/config"));
  if (!res.ok) throw new Error(`GET /config failed: ${res.status}`);
  return (await res.json()) as AppConfig;
}

// The backend's /status payload. `ready` is false until every configured model
// file and the inference binary are present; `missingModels` / `serverBinary`
// say exactly what's blocking, so the UI can show an actionable reason instead
// of waiting indefinitely.
export interface BackendStatus {
  status: string;
  ready: boolean;
  missingModels?: string[];
  chatModel?: string;
  embeddingModel?: string;
  serverBinary?: boolean;
  ramGb?: number;
  [key: string]: unknown;
}

export async function fetchStatus(): Promise<BackendStatus> {
  const res = await fetch(apiUrl("/status"));
  if (!res.ok) throw new Error(`GET /status failed: ${res.status}`);
  return (await res.json()) as BackendStatus;
}

// Model acquisition (D2). The GGUF chat model does not fit in the installer, so it
// arrives either by resumable download or from an offline pack on a USB/share. Both
// paths are gated by the real SHA256 in the backend's manifest; `bytes` here is only
// progress on disk, never a claim that a file was verified.
export interface ModelEntry {
  nombre: string;
  archivo: string;
  bytes: number;
  bytesTotal: number | null;
  presente: boolean;
  descargable: boolean;
  /** Cuál conviene en este equipo según su RAM. Recomendación, no restricción. */
  recomendado: boolean;
  /** Los dos modelos de chat son alternativas: con uno alcanza. */
  esChat: boolean;
}

export interface ModelsStatus {
  directorio: string;
  tarea: {
    estado: "inactivo" | "corriendo" | "listo" | "error";
    accion: "descarga" | "paquete" | null;
    archivo: string | null;
    resultado: Record<string, string> | null;
    error: string | null;
  };
  modelos: ModelEntry[];
}

export async function fetchModelsStatus(): Promise<ModelsStatus> {
  const res = await fetch(apiUrl("/models/status"));
  if (!res.ok) throw new Error(`GET /models/status failed: ${res.status}`);
  return (await res.json()) as ModelsStatus;
}

/** `archivo` elige qué modelo traer; sin él, lo mínimo para que responda. */
export async function startModelDownload(archivo?: string): Promise<ModelsStatus> {
  const res = await fetch(apiUrl("/models/fetch"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ archivo: archivo ?? null }),
  });
  if (res.status === 409) {
    throw new Error("Ya hay una obtención de modelos en curso.");
  }
  if (!res.ok) throw new Error(`POST /models/fetch failed: ${res.status}`);
  return (await res.json()) as ModelsStatus;
}

export async function installModelsFromPack(
  dir: string,
  archivo?: string,
): Promise<ModelsStatus> {
  const res = await fetch(apiUrl("/models/pack"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ dir, archivo: archivo ?? null }),
  });
  if (res.status === 400) {
    throw new Error("No se pudo leer esa carpeta. Verifica la ruta del paquete.");
  }
  if (res.status === 409) {
    throw new Error("Ya hay una obtención de modelos en curso.");
  }
  if (!res.ok) throw new Error(`POST /models/pack failed: ${res.status}`);
  return (await res.json()) as ModelsStatus;
}

export async function webSearch(query: string): Promise<SearchResult[]> {
  const res = await fetch(apiUrl("/search"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ query }),
  });
  if (res.status === 503) {
    throw new Error("La búsqueda web está desactivada en este equipo.");
  }
  if (res.status === 502) {
    throw new Error("La búsqueda web no respondió; intenta de nuevo más tarde.");
  }
  if (!res.ok) throw new Error(`POST /search failed: ${res.status}`);
  const data = (await res.json()) as { results: SearchResult[] };
  return data.results;
}

// Parses one SSE `data:` payload and dispatches to the matching handler.
function dispatchEvent(raw: string, handlers: StreamHandlers): boolean {
  let evt: {
    type?: string;
    content?: string;
    citations?: Citation[];
    message?: string;
    categories?: DisambiguationCategory[];
    pendingMessage?: string;
  };
  try {
    evt = JSON.parse(raw);
  } catch {
    return false;
  }
  switch (evt.type) {
    case "citations":
      handlers.onCitations?.(evt.citations ?? []);
      return false;
    case "disambiguate":
      handlers.onDisambiguate?.({
        message: evt.message ?? "",
        categories: evt.categories ?? [],
        pendingMessage: evt.pendingMessage ?? "",
      });
      return false;
    case "token":
      if (evt.content) handlers.onToken?.(evt.content);
      return false;
    case "error":
      handlers.onError?.(evt.message ?? "Error del modelo.");
      return true;
    case "done":
      return true;
    default:
      return false;
  }
}

export async function streamChat(
  message: string,
  history: ChatMessage[],
  handlers: StreamHandlers,
  category?: string,
): Promise<void> {
  const res = await fetch(apiUrl("/chat"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ message, history, category }),
    signal: handlers.signal,
  });

  if (!res.ok || !res.body) {
    handlers.onError?.(`El servicio de chat respondió ${res.status}.`);
    handlers.onDone?.();
    return;
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let finished = false;

  try {
    while (!finished) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });

      // SSE frames are separated by a blank line.
      let sep: number;
      while ((sep = buffer.indexOf("\n\n")) !== -1) {
        const frame = buffer.slice(0, sep);
        buffer = buffer.slice(sep + 2);
        for (const line of frame.split("\n")) {
          if (line.startsWith("data: ")) {
            if (dispatchEvent(line.slice(6), handlers)) {
              finished = true;
            }
          }
        }
      }
    }
  } catch (err) {
    if ((err as Error).name !== "AbortError") {
      handlers.onError?.((err as Error).message);
    }
  } finally {
    handlers.onDone?.();
  }
}
