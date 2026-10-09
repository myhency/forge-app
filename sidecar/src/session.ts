import { buildContent, type ContentBlock } from "./content.js";
import { buildSkillsPlugin } from "./skills_plugin.js";
import { turnUsage } from "./usage.js";
import type { SidecarCommand, SidecarEvent, WireAvailableModel, WireEffort, WirePermissionDecision, WireSlashCommand } from "./protocol.js";

export type QueryHandle = AsyncIterable<unknown> & {
  interrupt: () => Promise<unknown>;
  getContextUsage?: () => Promise<{ totalTokens: number; maxTokens: number }>;
  supportedCommands?: () => Promise<unknown>;
  setPermissionMode?: (mode: string) => Promise<unknown>;
  setModel?: (model?: string) => Promise<unknown>;
  applyFlagSettings?: (settings: Record<string, unknown>) => Promise<unknown>;
  supportedModels?: () => Promise<unknown>;
  stopTask?: (taskId: string) => Promise<void>;
};

export type QueryFn = (args: { prompt: AsyncIterable<unknown>; options: Record<string, unknown> }) => QueryHandle;

type PermissionResult =
  | { behavior: "allow"; updatedInput?: Record<string, unknown>; updatedPermissions?: unknown[] }
  | { behavior: "deny"; message: string };

export const DENIED_MESSAGE =
  "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). STOP what you are doing and wait for the user to tell you how to proceed.";
export const QUESTION_SKIPPED_MESSAGE = "The user skipped the question.";
const QUESTION_TOOL = "AskUserQuestion";

interface PendingPermission {
  resolve: (result: PermissionResult) => void;
  suggestions?: unknown[];
  toolName: string;
  input: Record<string, unknown>;
}

const CONTEXT_USAGE_TIMEOUT_MS = 3000;
const COMMANDS_TIMEOUT_MS = 10000;

// The SDK may list a name with or without its slash; Forge keeps it without.
function toWireCommands(raw: unknown): WireSlashCommand[] {
  if (!Array.isArray(raw)) return [];
  const seen = new Set<string>();
  const commands: WireSlashCommand[] = [];
  for (const item of raw) {
    const entry = typeof item === "string" ? { name: item } : (item as { name?: unknown } | null);
    const name = typeof entry?.name === "string" ? entry.name.replace(/^\//, "") : "";
    if (name === "" || seen.has(name)) continue;
    seen.add(name);
    const command: WireSlashCommand = { name };
    const { description, argumentHint } = entry as { description?: unknown; argumentHint?: unknown };
    if (typeof description === "string" && description !== "") command.description = description;
    if (typeof argumentHint === "string" && argumentHint !== "") command.argument_hint = argumentHint;
    commands.push(command);
  }
  return commands;
}

const EFFORT_LEVELS: WireEffort[] = ["low", "medium", "high", "xhigh", "max"];

// The SDK's model list as Forge wants it: the levels are cut to the ones Forge knows, and a model
// that takes no effort has none.
function toWireModels(raw: unknown): WireAvailableModel[] {
  if (!Array.isArray(raw)) return [];
  const models: WireAvailableModel[] = [];
  for (const item of raw) {
    const model = item as { value?: unknown; resolvedModel?: unknown; displayName?: unknown; supportsEffort?: unknown; supportedEffortLevels?: unknown } | null;
    if (typeof model?.value !== "string" || model.value === "") continue;
    const offered = model.supportsEffort === false || !Array.isArray(model.supportedEffortLevels) ? [] : model.supportedEffortLevels;
    models.push({
      value: model.value,
      ...(typeof model.resolvedModel === "string" && model.resolvedModel !== "" ? { resolved_model: model.resolvedModel } : {}),
      display_name: typeof model.displayName === "string" && model.displayName !== "" ? model.displayName : model.value,
      effort_levels: EFFORT_LEVELS.filter((level) => offered.includes(level)),
    });
  }
  return models;
}

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T | undefined> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => resolve(undefined), ms);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}

function toSdkPermissionMode(mode: string): "default" | "acceptEdits" | "bypassPermissions" | "plan" | "auto" {
  switch (mode) {
    case "accept_edits":
      return "acceptEdits";
    case "bypass_permissions":
      return "bypassPermissions";
    case "plan":
      return "plan";
    case "auto":
      return "auto";
    default:
      return "default";
  }
}

/** The `allowedTools` strings an "always allow" would add: `Read`, `Bash(npm test:*)`. */
export function rulesFromSuggestions(suggestions: unknown[] | undefined): string[] {
  const rules: string[] = [];
  for (const update of suggestions ?? []) {
    const item = update as { type?: string; behavior?: string; rules?: Array<{ toolName: string; ruleContent?: string }> };
    if (item.type !== "addRules" || item.behavior !== "allow") continue;
    for (const rule of item.rules ?? []) {
      rules.push(rule.ruleContent ? `${rule.toolName}(${rule.ruleContent})` : rule.toolName);
    }
  }
  return rules;
}

function decisionToResult(decision: WirePermissionDecision, suggestions: unknown[] | undefined, toolName: string): PermissionResult {
  if (decision === "deny") {
    return { behavior: "deny", message: toolName === QUESTION_TOOL ? QUESTION_SKIPPED_MESSAGE : DENIED_MESSAGE };
  }
  if (decision === "allow_always") {
    return { behavior: "allow", updatedPermissions: suggestions };
  }
  return { behavior: "allow" };
}

export class SidecarSession {
  private queuedMessages: unknown[] = [];
  private waitingResolvers: Array<(msg: unknown) => void> = [];
  private pendingPermissions = new Map<string, PendingPermission>();
  private abortController = new AbortController();
  private started = false;
  private shuttingDown = false;
  private activeQuery: QueryHandle | null = null;
  private sessionEndedPromise: Promise<void> = Promise.resolve();
  // The model of the current turn's main-loop assistant messages: `result.usage` is that model's.
  private mainModel: string | null = null;
  // Web searches the WebSearch tool reported since the last result (that tool's own request is not in `result.usage`).
  private pendingSearches = 0;
  private lastCommandsKey: string | null = null;
  private knownCommands = new Map<string, WireSlashCommand>();
  private readonly contextUsageTimeoutMs: number;
  private readonly commandsTimeoutMs: number;

  constructor(
    private readonly queryFn: QueryFn,
    private readonly emit: (event: SidecarEvent) => void,
    options: { contextUsageTimeoutMs?: number; commandsTimeoutMs?: number } = {},
  ) {
    this.contextUsageTimeoutMs = options.contextUsageTimeoutMs ?? CONTEXT_USAGE_TIMEOUT_MS;
    this.commandsTimeoutMs = options.commandsTimeoutMs ?? COMMANDS_TIMEOUT_MS;
  }

  async handle(command: SidecarCommand): Promise<void> {
    switch (command.type) {
      case "start_session":
        await this.start(command);
        return;
      case "send_message":
        this.pushUserMessage(buildContent(command.text, command.attachments ?? []));
        return;
      case "permission_response": {
        const pending = this.pendingPermissions.get(command.permission_request_id);
        if (!pending) return;
        pending.resolve(decisionToResult(command.decision, pending.suggestions, pending.toolName));
        this.pendingPermissions.delete(command.permission_request_id);
        return;
      }
      case "answer_question": {
        const pending = this.pendingPermissions.get(command.permission_request_id);
        if (!pending || pending.toolName !== QUESTION_TOOL) return;
        pending.resolve({ behavior: "allow", updatedInput: { ...pending.input, answers: command.answers } });
        this.pendingPermissions.delete(command.permission_request_id);
        return;
      }
      case "interrupt":
        if (this.activeQuery) {
          await this.activeQuery.interrupt();
        }
        return;
      case "set_permission_mode":
        await this.change(
          (query) => query.setPermissionMode?.(toSdkPermissionMode(command.mode)),
          { type: "permission_mode_set", mode: command.mode },
        );
        return;
      case "set_model":
        await this.change(
          (query) => query.setModel?.(command.model ?? undefined),
          { type: "model_set", model: command.model },
        );
        return;
      case "set_effort":
        await this.change(
          (query) =>
            query.applyFlagSettings
              ? query.applyFlagSettings({ effortLevel: command.effort })
              : Promise.reject(new Error("this version of the SDK cannot change the effort")),
          { type: "effort_set", effort: command.effort },
        );
        return;
      case "stop_task": {
        const query = this.activeQuery;
        if (!query?.stopTask) {
          this.emit({ type: "setting_failed", message: "no session is running" });
          return;
        }
        try {
          await query.stopTask(command.task_id);
        } catch (err) {
          this.emit({ type: "setting_failed", message: err instanceof Error ? err.message : String(err) });
        }
        return;
      }
      case "shutdown":
        this.shuttingDown = true;
        this.abortController.abort();
        for (const pending of this.pendingPermissions.values()) {
          pending.resolve({ behavior: "deny", message: "Sidecar is shutting down" });
        }
        this.pendingPermissions.clear();
        return;
    }
  }

  // Applies a change to the running query and answers with `done`, or with `setting_failed` when
  // there is no query or the SDK refuses. Nothing here ends the session.
  private async change(apply: (query: QueryHandle) => Promise<unknown> | undefined, done: SidecarEvent): Promise<void> {
    const query = this.activeQuery;
    if (!query) {
      this.emit({ type: "setting_failed", message: "no session is running" });
      return;
    }
    try {
      await apply(query);
    } catch (err) {
      this.emit({ type: "setting_failed", message: err instanceof Error ? err.message : String(err) });
      return;
    }
    this.emit(done);
  }

  async waitUntilEnded(): Promise<void> {
    await this.sessionEndedPromise;
  }

  private async *userMessages(): AsyncGenerator<unknown> {
    while (!this.abortController.signal.aborted) {
      yield await this.nextUserMessage();
    }
  }

  private nextUserMessage(): Promise<unknown> {
    const queued = this.queuedMessages.shift();
    if (queued !== undefined) return Promise.resolve(queued);
    return new Promise((resolve) => this.waitingResolvers.push(resolve));
  }

  private pushUserMessage(content: string | ContentBlock[]): void {
    const message = { type: "user", message: { role: "user", content }, parent_tool_use_id: null };
    const resolver = this.waitingResolvers.shift();
    if (resolver) {
      resolver(message);
    } else {
      this.queuedMessages.push(message);
    }
  }

  private async start(command: Extract<SidecarCommand, { type: "start_session" }>): Promise<void> {
    if (this.started) return;
    this.started = true;

    // Covers the whole start (building the skills plugin included), so a shutdown that arrives
    // early still waits for the cleanup before the process exits.
    const run = this.run(command);
    this.sessionEndedPromise = run.then(
      () => undefined,
      () => undefined,
    );
    await run;
  }

  private async run(command: Extract<SidecarCommand, { type: "start_session" }>): Promise<void> {
    const canUseTool = async (
      toolName: string,
      input: Record<string, unknown>,
      options: { requestId: string; signal: AbortSignal; suggestions?: unknown[]; [key: string]: unknown },
    ): Promise<PermissionResult> => {
      const permissionRequestId = options.requestId;
      this.emit({
        type: "permission_request",
        permission_request_id: permissionRequestId,
        tool_name: toolName,
        input,
        rules: rulesFromSuggestions(options.suggestions),
      });
      return new Promise<PermissionResult>((resolve) => {
        this.pendingPermissions.set(permissionRequestId, { resolve, suggestions: options.suggestions, toolName, input });
        // The real SDK always supplies `signal`; the optional chain only tolerates test doubles that omit it.
        options.signal?.addEventListener(
          "abort",
          () => {
            if (this.pendingPermissions.has(permissionRequestId)) {
              this.pendingPermissions.delete(permissionRequestId);
              resolve({ behavior: "deny", message: "Interrupted" });
            }
          },
          { once: true },
        );
      });
    };

    // Forge's toggles are the only source of truth (spec §17.1). The `??` guards a command from an
    // older parent that omits the fields: that must mean nothing enabled, never the SDK's defaults.
    const skillsPlugin = await buildSkillsPlugin(command.skills ?? {});
    if (this.shuttingDown) {
      await skillsPlugin?.cleanup().catch(() => {});
      this.emit({ type: "session_ended" });
      return;
    }
    const plugins = [
      ...(command.plugins ?? []).map((path) => ({ type: "local" as const, path })),
      ...(skillsPlugin ? [{ type: "local" as const, path: skillsPlugin.path }] : []),
    ];

    let query: QueryHandle;
    try {
      query = this.queryFn({
        prompt: this.userMessages(),
        options: {
          cwd: command.cwd,
          resume: command.resume_session_id ?? undefined,
          permissionMode: toSdkPermissionMode(command.permission_mode),
          model: command.model ?? undefined,
          effort: command.effort ?? undefined,
          abortController: this.abortController,
          canUseTool,
          includePartialMessages: true,
          // The SDK refuses bypassPermissions later unless this is set now. It only lets the mode be
          // chosen; Forge enters it only when the person confirms, and only when Settings allows it.
          allowDangerouslySkipPermissions: command.allow_bypass,
          // The SDK sends thinking with its text omitted unless asked for a summary.
          thinking: { type: "adaptive", display: "summarized" },
          settingSources: ["project"],
          strictMcpConfig: true,
          skills: skillsPlugin?.skills ?? [],
          plugins,
          // The output style of a new session; the SDK's default style when none is set.
          ...(command.output_style ? { settings: { outputStyle: command.output_style } } : {}),
          mcpServers: command.mcp_servers ?? {},
          additionalDirectories: command.additional_directories ?? [],
          allowedTools: command.allowed_tools ?? [],
          // Forge shows a stop button per background task and predicts the next prompt unless Settings turns it off.
          promptSuggestions: command.prompt_suggestions,
          perTaskStopAffordance: true,
        },
      });
    } catch (err) {
      await skillsPlugin?.cleanup().catch(() => {});
      throw err;
    }
    this.activeQuery = query;
    void this.emitSupportedCommands();
    void this.emitSupportedModels();

    try {
      for await (const message of query) {
        this.forwardMessage(message);
      }
      this.emit({ type: "session_ended" });
    } catch (err) {
      if (this.shuttingDown) {
        this.emit({ type: "session_ended" });
      } else {
        this.emit({ type: "error", message: err instanceof Error ? err.message : String(err) });
      }
    } finally {
      // A failing cleanup must not replace the session's real outcome.
      await skillsPlugin?.cleanup().catch(() => {});
    }
  }

  // `init` lists names only, so a name the earlier list already described keeps its description and hint
  // (the two lists also differ in order and sometimes in length). Sent only when the result differs from
  // the last one, compared without regard to order.
  private emitCommands(raw: unknown, namesOnly = false): void {
    let commands = toWireCommands(raw);
    if (namesOnly) commands = commands.map((c) => this.knownCommands.get(c.name) ?? c);
    const key = JSON.stringify([...commands].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0)));
    if (this.shuttingDown || commands.length === 0 || key === this.lastCommandsKey) return;
    this.lastCommandsKey = key;
    this.knownCommands = new Map(commands.map((c) => [c.name, c]));
    this.emit({ type: "commands_available", commands });
  }

  // A failure or a slow answer only means no list yet; it never touches the session.
  private async emitSupportedCommands(): Promise<void> {
    const query = this.activeQuery;
    if (!query?.supportedCommands) return;
    try {
      this.emitCommands(await withTimeout(query.supportedCommands(), this.commandsTimeoutMs));
    } catch {
      // No list from this call; `init` may still bring one.
    }
  }

  // A failure, a slow answer or an empty list only means no model list; it never touches the session.
  private async emitSupportedModels(): Promise<void> {
    const query = this.activeQuery;
    if (!query?.supportedModels) return;
    try {
      const models = toWireModels(await withTimeout(query.supportedModels(), this.commandsTimeoutMs));
      if (this.shuttingDown || models.length === 0) return;
      this.emit({ type: "models_available", models });
    } catch {
      // No list from this call.
    }
  }

  // A failure or a slow answer only means no ring value this turn; it never touches the turn.
  private async emitContextUsage(): Promise<void> {
    const query = this.activeQuery;
    if (!query?.getContextUsage) return;
    try {
      const context = await withTimeout(query.getContextUsage(), this.contextUsageTimeoutMs);
      if (this.shuttingDown || !context) return;
      const { totalTokens, maxTokens } = context;
      if (Number.isFinite(totalTokens) && Number.isFinite(maxTokens) && maxTokens > 0 && totalTokens >= 0) {
        this.emit({ type: "context_usage", total_tokens: totalTokens, max_tokens: maxTokens });
      }
    } catch {
      // No context value this turn.
    }
  }

  private forwardMessage(message: any): void {
    switch (message.type) {
      case "system":
        if (message.subtype === "init") {
          this.emit({ type: "session_started", session_id: message.session_id });
          this.emitCommands(message.slash_commands, true);
        } else if (message.subtype === "background_tasks_changed") {
          const tasks = (Array.isArray(message.tasks) ? message.tasks : [])
            .filter((task: any) => task && !task.ambient && typeof task.task_id === "string" && task.task_id !== "")
            .map((task: any) => ({
              task_id: task.task_id as string,
              description: typeof task.description === "string" ? task.description : "",
              task_type: typeof task.task_type === "string" ? task.task_type : "",
            }));
          this.emit({ type: "background_tasks", tasks });
        } else if (message.subtype === "task_notification") {
          const status = message.status;
          if (typeof message.task_id === "string" && (status === "completed" || status === "failed" || status === "stopped")) {
            this.emit({ type: "task_finished", task_id: message.task_id, status, summary: typeof message.summary === "string" ? message.summary : "" });
          }
        }
        return;
      case "prompt_suggestion":
        if (typeof message.suggestion === "string" && message.suggestion !== "") {
          this.emit({ type: "prompt_suggestion", text: message.suggestion });
        }
        return;
      case "stream_event": {
        const event = message.event;
        if (event?.type === "content_block_delta" && event.delta?.type === "text_delta") {
          this.emit({ type: "assistant_delta", text: event.delta.text });
        } else if (event?.type === "content_block_delta" && event.delta?.type === "thinking_delta" && typeof event.delta.thinking === "string") {
          this.emit({ type: "thinking_delta", text: event.delta.thinking });
        }
        return;
      }
      case "assistant": {
        const model = message.message?.model;
        if (!message.parent_tool_use_id && typeof model === "string" && model !== "" && !model.startsWith("<")) {
          this.mainModel = model;
        }
        const blocks = message.message?.content;
        if (!Array.isArray(blocks)) return;
        for (const block of blocks) {
          if (block?.type === "tool_use") {
            this.emit({ type: "tool_use", tool_call_id: block.id, name: block.name, input: block.input });
          }
        }
        return;
      }
      case "user": {
        const searches = message.tool_use_result?.searchCount;
        if (!message.parent_tool_use_id && typeof searches === "number" && Number.isFinite(searches) && searches > 0) {
          this.pendingSearches += Math.floor(searches);
        }
        const blocks = message.message?.content;
        if (!Array.isArray(blocks)) return;
        for (const block of blocks) {
          if (block?.type === "tool_result") {
            this.emit({
              type: "tool_result",
              tool_call_id: block.tool_use_id,
              output: extractTextContent(block.content),
              is_error: Boolean(block.is_error),
            });
          }
        }
        return;
      }
      case "result": {
        // `result.usage` is this turn's main-loop usage. `modelUsage` is not used: it is a running
        // total, includes helper models and sub-agents, and after a resume already holds the
        // earlier turns (spec §20.10).
        this.emit({ type: "turn_complete", usage: turnUsage(this.mainModel, message.usage, this.pendingSearches) });
        this.mainModel = null;
        this.pendingSearches = 0;
        void this.emitContextUsage();
        return;
      }
      default:
        return;
    }
  }
}

function extractTextContent(content: unknown): string {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .filter((block): block is { type: "text"; text: string } => typeof block === "object" && block !== null && (block as { type?: unknown }).type === "text")
    .map((block) => block.text)
    .join("");
}
