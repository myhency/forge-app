import type { SidecarCommand, SidecarEvent } from "./protocol.js";

export type QueryFn = (args: { prompt: AsyncIterable<unknown>; options: Record<string, unknown> }) => AsyncIterable<any>;

type PermissionResult =
  | { behavior: "allow"; updatedInput?: Record<string, unknown> }
  | { behavior: "deny"; message: string };

export class SidecarSession {
  private queuedMessages: unknown[] = [];
  private waitingResolvers: Array<(msg: unknown) => void> = [];
  private pendingPermissions = new Map<string, (result: PermissionResult) => void>();
  private abortController = new AbortController();
  private started = false;

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
        const resolver = this.pendingPermissions.get(command.permission_request_id);
        if (!resolver) return;
        resolver(
          command.decision === "deny"
            ? { behavior: "deny", message: "Denied by user" }
            : { behavior: "allow" },
        );
        this.pendingPermissions.delete(command.permission_request_id);
        return;
      }
      case "interrupt":
        this.abortController.abort();
        return;
      case "shutdown":
        this.abortController.abort();
        for (const resolver of this.pendingPermissions.values()) {
          resolver({ behavior: "deny", message: "Sidecar is shutting down" });
        }
        this.pendingPermissions.clear();
        return;
    }
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

    const canUseTool = async (
      toolName: string,
      input: Record<string, unknown>,
      options: { requestId: string; [key: string]: unknown },
    ): Promise<PermissionResult> => {
      const permissionRequestId = options.requestId;
      this.emit({
        type: "permission_request",
        permission_request_id: permissionRequestId,
        tool_name: toolName,
        input,
      });
      return new Promise<PermissionResult>((resolve) => {
        this.pendingPermissions.set(permissionRequestId, resolve);
      });
    };

    const stream = this.queryFn({
      prompt: this.userMessages(),
      options: {
        cwd: command.cwd,
        resume: command.resume_session_id ?? undefined,
        permissionMode: command.permission_mode,
        model: command.model ?? undefined,
        abortController: this.abortController,
        canUseTool,
      },
    });

    try {
      for await (const message of stream) {
        this.forwardMessage(message);
      }
      this.emit({ type: "session_ended" });
    } catch (err) {
      this.emit({ type: "error", message: err instanceof Error ? err.message : String(err) });
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
