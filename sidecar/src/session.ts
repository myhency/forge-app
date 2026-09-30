import { buildSkillsPlugin } from "./skills_plugin.js";
import type { SidecarCommand, SidecarEvent, WirePermissionDecision } from "./protocol.js";

export type QueryHandle = AsyncIterable<unknown> & {
  interrupt: () => Promise<unknown>;
};

export type QueryFn = (args: { prompt: AsyncIterable<unknown>; options: Record<string, unknown> }) => QueryHandle;

type PermissionResult =
  | { behavior: "allow"; updatedInput?: Record<string, unknown>; updatedPermissions?: unknown[] }
  | { behavior: "deny"; message: string };

interface PendingPermission {
  resolve: (result: PermissionResult) => void;
  suggestions?: unknown[];
}

function toSdkPermissionMode(mode: string): "default" | "acceptEdits" | "bypassPermissions" | "plan" {
  switch (mode) {
    case "accept_edits":
      return "acceptEdits";
    case "bypass_permissions":
      return "bypassPermissions";
    case "plan":
      return "plan";
    default:
      return "default";
  }
}

function decisionToResult(decision: WirePermissionDecision, suggestions: unknown[] | undefined): PermissionResult {
  if (decision === "deny") {
    return { behavior: "deny", message: "Denied by user" };
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

  constructor(
    private readonly queryFn: QueryFn,
    private readonly emit: (event: SidecarEvent) => void,
  ) {}

  async handle(command: SidecarCommand): Promise<void> {
    switch (command.type) {
      case "start_session":
        await this.start(command);
        return;
      case "send_message":
        this.pushUserMessage(command.text);
        return;
      case "permission_response": {
        const pending = this.pendingPermissions.get(command.permission_request_id);
        if (!pending) return;
        pending.resolve(decisionToResult(command.decision, pending.suggestions));
        this.pendingPermissions.delete(command.permission_request_id);
        return;
      }
      case "interrupt":
        if (this.activeQuery) {
          await this.activeQuery.interrupt();
        }
        return;
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

  private pushUserMessage(text: string): void {
    const message = { type: "user", message: { role: "user", content: text }, parent_tool_use_id: null };
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
      });
      return new Promise<PermissionResult>((resolve) => {
        this.pendingPermissions.set(permissionRequestId, { resolve, suggestions: options.suggestions });
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
          abortController: this.abortController,
          canUseTool,
          includePartialMessages: true,
          settingSources: ["project"],
          strictMcpConfig: true,
          skills: skillsPlugin?.skills ?? [],
          plugins,
          mcpServers: command.mcp_servers ?? {},
        },
      });
    } catch (err) {
      await skillsPlugin?.cleanup().catch(() => {});
      throw err;
    }
    this.activeQuery = query;

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

  private forwardMessage(message: any): void {
    switch (message.type) {
      case "system":
        if (message.subtype === "init") {
          this.emit({ type: "session_started", session_id: message.session_id });
        }
        return;
      case "stream_event": {
        const event = message.event;
        if (event?.type === "content_block_delta" && event.delta?.type === "text_delta") {
          this.emit({ type: "assistant_delta", text: event.delta.text });
        }
        return;
      }
      case "assistant": {
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
      case "result":
        this.emit({
          type: "turn_complete",
          input_tokens: message.usage?.input_tokens ?? 0,
          output_tokens: message.usage?.output_tokens ?? 0,
        });
        return;
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
