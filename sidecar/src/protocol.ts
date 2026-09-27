export type WirePermissionMode = "default" | "accept_edits" | "bypass_permissions" | "plan";
export type WirePermissionDecision = "allow_once" | "allow_always" | "deny";

export type SidecarCommand =
  | { type: "start_session"; id: string; cwd: string; resume_session_id: string | null; permission_mode: WirePermissionMode; model: string | null }
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
  | { type: "turn_complete"; input_tokens: number; output_tokens: number }
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
