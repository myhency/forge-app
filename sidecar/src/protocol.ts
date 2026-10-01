export type WirePermissionMode = "default" | "accept_edits" | "bypass_permissions" | "plan";
export type WirePermissionDecision = "allow_once" | "allow_always" | "deny";

export interface WireTokenUsage {
  input: number;
  output: number;
  cache_read: number;
  cache_write_5m: number;
  cache_write_1h: number;
  web_search_requests: number;
}

export interface WireSlashCommand {
  name: string;
  description?: string;
  argument_hint?: string;
}

export type SidecarCommand =
  | {
      type: "start_session";
      id: string;
      cwd: string;
      resume_session_id: string | null;
      permission_mode: WirePermissionMode;
      model: string | null;
      skills: Record<string, string>;
      plugins: string[];
      mcp_servers: Record<string, unknown>;
    }
  | { type: "send_message"; id: string; text: string }
  | { type: "permission_response"; id: string; permission_request_id: string; decision: WirePermissionDecision }
  | { type: "interrupt"; id: string }
  | { type: "shutdown"; id: string };

export type SidecarEvent =
  | { type: "session_started"; session_id: string }
  | { type: "assistant_delta"; text: string }
  | { type: "tool_use"; tool_call_id: string; name: string; input: unknown }
  | { type: "tool_result"; tool_call_id: string; output: string; is_error: boolean }
  | { type: "permission_request"; permission_request_id: string; tool_name: string; input: unknown }
  | { type: "turn_complete"; usage: Record<string, WireTokenUsage> }
  | { type: "context_usage"; total_tokens: number; max_tokens: number }
  | { type: "commands_available"; commands: WireSlashCommand[] }
  | { type: "error"; message: string }
  | { type: "session_ended" };

export function parseCommand(line: string): SidecarCommand {
  const parsed: unknown = JSON.parse(line);
  if (typeof parsed !== "object" || parsed === null || typeof (parsed as { type?: unknown }).type !== "string") {
    throw new Error(`invalid sidecar command: ${line}`);
  }
  return parsed as SidecarCommand;
}

export function serializeEvent(event: SidecarEvent): string {
  return JSON.stringify(event);
}
