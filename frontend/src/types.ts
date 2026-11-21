// Rust (gan-core) tipleriyle birebir eşleşen frontend tipleri.
// Serileştirme biçimi serde tarafından belirlendiği için alan adları snake_case.

export type MsgRole = "user" | "assistant" | "system";
export type Theme = "system" | "light" | "dark";

export interface FileRef {
  name: string;
  url: string;
}

export interface Account {
  id: string;
  name: string;
  /** ARGB renk (u32). */
  color: number;
  created_at: number;
  model?: string;
  system_prompt?: string;
}

export interface Msg {
  id: number;
  role: MsgRole;
  text: string;
  thinking: string;
  files: FileRef[];
  done: boolean;
  created_at: number;
  model?: string;
  error?: string;
}

export interface AppSettings {
  api_url: string;
  default_model: string;
  temperature: number;
  max_tokens?: number;
  thinking_enabled: boolean;
  reasoning_effort: string;
  system_prompt: string;
  theme: Theme;
}

/** `gan-stream` olayları. Rust tarafında `#[serde(tag = "kind")]`. */
export type StreamEvent =
  | { kind: "started"; account_id: string; msg_id: number; model: string }
  | { kind: "delta"; account_id: string; msg_id: number; content?: string; reasoning?: string }
  | { kind: "done"; account_id: string; msg_id: number; final_text: string; final_reasoning: string }
  | { kind: "cancelled"; account_id: string; msg_id: number; partial: string }
  | { kind: "error"; account_id: string; msg_id: number; error: string };

/** ARGB u32'yi CSS renk dizesine çevir. */
export function argbToCss(argb: number): string {
  const a = (argb >>> 24) & 0xff;
  const r = (argb >>> 16) & 0xff;
  const g = (argb >>> 8) & 0xff;
  const b = argb & 0xff;
  if (a === 255) return `#${hex(r)}${hex(g)}${hex(b)}`;
  return `rgba(${r}, ${g}, ${b}, ${(a / 255).toFixed(3)})`;
}

function hex(n: number): string {
  return n.toString(16).padStart(2, "0");
}
