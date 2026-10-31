export type WirePermissionMode = "default" | "accept_edits" | "bypass_permissions" | "plan" | "auto";
export type WireEffort = "low" | "medium" | "high" | "xhigh" | "max";

export interface WireAvailableModel {
  // What the model is called in the SDK's list (`opus`, `default`, or a full id).
  value: string;
  // The full id it stands for, when the SDK says; a session's model id is matched against both.
  resolved_model?: string;
  display_name: string;
  effort_levels: WireEffort[];
}

export type WirePermissionDecision = "allow_once" | "allow_always" | "deny";

export interface WireTokenUsage {
  input: number;
  output: number;
  cache_read: number;
  cache_write_5m: number;
  cache_write_1h: number;
  web_search_requests: number;
}

export interface WireAttachment {
  kind: "image" | "file";
  path: string;
}

export interface WireSlashCommand {
  name: string;
  description?: string;
  argument_hint?: string;
}

export interface WireBackgroundTask {
  task_id: string;
  description: string;
  task_type: string;
}

export type SidecarCommand =
  | {
      type: "start_session";
      id: string;
      cwd: string;
      resume_session_id: string | null;
      permission_mode: WirePermissionMode;
      model: string | null;
      effort?: WireEffort | null;
      skills: Record<string, string>;
      plugins: string[];
      mcp_servers: Record<string, unknown>;
      additional_directories?: string[];
      allowed_tools?: string[];
      env?: Record<string, string>;
      output_style?: string;
      prompt_suggestions: boolean;
      allow_bypass: boolean;
      dynamic_workflows: boolean;
      // Offers Claude the update_plan and wrap_up tools (a parent that omits it means off).
      turn_tools?: boolean;
      // Offers Claude the generate_sandboxed_ui tool (a parent that omits it means off).
      generative_ui?: boolean;
    }
  | { type: "send_message"; id: string; text: string; attachments?: WireAttachment[] }
  | { type: "permission_response"; id: string; permission_request_id: string; decision: WirePermissionDecision; reason?: string }
  | { type: "answer_question"; id: string; permission_request_id: string; answers: Record<string, string> }
  | { type: "stop_task"; id: string; task_id: string }
  | { type: "interrupt"; id: string }
  // A question about the conversation that is not added to it (/btw); answered by a side_answer.
  | { type: "side_question"; id: string; request_id: string; question: string }
  // Puts the files back as they were before the user message `message_uuid`; with dry_run only says what would change.
  | { type: "rewind_files"; id: string; request_id: string; message_uuid: string; dry_run: boolean }
  | { type: "set_permission_mode"; id: string; mode: WirePermissionMode }
  | { type: "set_model"; id: string; model: string | null }
  | { type: "set_effort"; id: string; effort: WireEffort | null }
  | { type: "shutdown"; id: string };

export type SidecarEvent =
  | { type: "session_started"; session_id: string }
  | { type: "assistant_delta"; text: string }
  | { type: "thinking_delta"; text: string }
  | { type: "tool_use"; tool_call_id: string; name: string; input: unknown }
  | { type: "tool_result"; tool_call_id: string; output: string; is_error: boolean }
  | { type: "permission_request"; permission_request_id: string; tool_name: string; input: unknown; rules?: string[] }
  // A request the SDK withdrew (the turn was interrupted); the parent stops showing it.
  | { type: "permission_cancelled"; permission_request_id: string }
  | { type: "turn_complete"; usage: Record<string, WireTokenUsage> }
  | { type: "context_usage"; total_tokens: number; max_tokens: number }
  | {
      type: "context_detail";
      categories: Array<{ name: string; tokens: number; kind: string }>;
      mcp_tools: Array<{ server: string; name: string; tokens: number }>;
      agents: Array<{ name: string; tokens: number }>;
    }
  | {
      type: "mcp_status";
      servers: Array<{
        name: string;
        status: string;
        scope: string | null;
        source: string | null;
        type: string | null;
        tool_count: number;
        error: string | null;
      }>;
    }
  | { type: "commands_available"; commands: WireSlashCommand[] }
  // The answer to set_permission_mode / set_model: what the SDK accepted, or why it did not. A
  // refusal is not an `error`: that ends the session.
  | { type: "permission_mode_set"; mode: WirePermissionMode }
  | { type: "model_set"; model: string | null }
  | { type: "effort_set"; effort: WireEffort | null }
  // The SDK's full set of background tasks (replace semantics), a finished one, and a predicted next prompt.
  | { type: "background_tasks"; tasks: WireBackgroundTask[] }
  | { type: "task_progress"; task_id: string; total_tokens: number }
  | {
      type: "task_finished";
      task_id: string;
      status: "completed" | "failed" | "stopped";
      summary: string;
      // What the SDK's notification reported; a Bash command reports neither.
      tokens?: number;
      duration_ms?: number;
    }
  | { type: "prompt_suggestion"; text: string }
  // The conversation was compacted; the token counts before and after, when the SDK says.
  | { type: "compacted"; pre_tokens?: number; post_tokens?: number }
  // The SDK wrote its "[Request interrupted by user…]" line: the person stopped the turn.
  | { type: "interrupted" }
  // The account's usage limit state the SDK reports: allowed, allowed_warning or rejected.
  | { type: "rate_limit"; status: string; kind?: string; percent?: number; resets_at?: number }
  // A short name for a new session, made from its first message.
  | { type: "session_title"; title: string }
  // The answer to a side_question: the text, or why there is none.
  | { type: "side_answer"; request_id: string; answer?: string; error?: string }
  // The id the sidecar gave the user message it just handed to the SDK (the SDK records it as the message's uuid).
  | { type: "user_message"; uuid: string }
  | {
      type: "rewind_result";
      request_id: string;
      can_rewind: boolean;
      error?: string;
      files_changed?: string[];
      insertions?: number;
      deletions?: number;
    }
  // The models the SDK lists for this session and the effort levels each takes; sent once after the start.
  | { type: "models_available"; models: WireAvailableModel[] }
  | { type: "setting_failed"; message: string }
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
