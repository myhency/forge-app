# Agent SDK type notes (v0.3.283)

Recorded by reading `node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts` directly
(this is the package's actual main entry — see `package.json`: `"main": "sdk.mjs"`,
`"types": "sdk.d.ts"`. There is no `dist/` subdirectory in this package; the `.d.ts`
files sit at the package root alongside `sdk.mjs`).

All `find` results:
```
node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts            (9833 lines — main entry)
node_modules/@anthropic-ai/claude-agent-sdk/bridge.d.ts          (388 lines)
node_modules/@anthropic-ai/claude-agent-sdk/browser-sdk.d.ts     (184 lines)
node_modules/@anthropic-ai/claude-agent-sdk/extractFromBunfs.d.ts (1 line)
node_modules/@anthropic-ai/claude-agent-sdk/core.d.ts            (20 lines)
node_modules/@anthropic-ai/claude-agent-sdk/sdk-tools.d.ts       (4153 lines)
node_modules/@anthropic-ai/claude-agent-sdk/agentSdkTypes.d.ts   (1 line)
```
Everything below is from `sdk.d.ts` unless noted.

## `query()` function signature (sdk.d.ts:3237-3240)

```typescript
export declare function query(_params: {
    prompt: string | AsyncIterable<SDKUserMessage>;
    options?: Options;
}): Query;
```

`Query` (sdk.d.ts:2843) is `AsyncGenerator<SDKMessage, void>` (plus some extra
control methods not needed for basic consumption).

So: when streaming input, the `prompt` iterable yields `SDKUserMessage` values
(not a wrapper, not `SDKUserMessageReplay`).

## SDKUserMessage (sdk.d.ts:6149-6235)

```typescript
export declare type SDKUserMessage = {
    type: 'user';
    // An Anthropic Messages API user message: a MessageParam with role "user"
    // whose content is a string or an array of content blocks (text, image,
    // document, tool_result, ...).
    message: MessageParam;          // imported from '@anthropic-ai/sdk/resources'
    parent_tool_use_id: string | null;
    isSynthetic?: boolean;
    tool_use_result?: unknown;
    priority?: 'now' | 'next' | 'later';
    origin?: SDKMessageOrigin;
    shouldQuery?: boolean;
    timestamp?: string;
    client_composed?: true;
    uuid?: UUID;
    pasted_content?: MessageParam['content'][];
    inline_pastes?: string[];
    session_id?: string;
    subagent_type?: string;
    task_description?: string;
};
```

Minimum required fields to construct one by hand: `type: 'user'`, `message`
(a `MessageParam`), `parent_tool_use_id` (use `null` for a top-level user
turn — it's required, not optional, but accepts `null`). Everything else is
optional.

There is also `SDKUserMessageReplay` (sdk.d.ts:6237-6308), used for replayed
history — same shape but `uuid`, `session_id`, `isReplay: true` are required
(not optional) and it adds `file_attachments?: unknown[]`. This is NOT what
`prompt`'s `AsyncIterable` expects — that's `SDKUserMessage` only.

## SDKMessage union (sdk.d.ts:5273)

Full union (as of 0.3.283 — much larger than the four-or-five variants a
reader might expect from older SDK docs):

```
SDKAssistantMessage | SDKUserMessage | SDKUserMessageReplay | SDKResultMessage
| SDKSystemMessage | SDKPartialAssistantMessage | SDKCompactBoundaryMessage
| SDKStatusMessage | SDKAPIRetryMessage | SDKControlRequestProgressMessage
| SDKModelRefusalFallbackMessage | SDKModelRefusalNoFallbackMessage
| SDKLocalCommandOutputMessage | SDKHookStartedMessage | SDKHookProgressMessage
| SDKHookResponseMessage | SDKPluginInstallMessage | SDKToolProgressMessage
| SDKAuthStatusMessage | SDKTaskNotificationMessage | SDKTaskStartedMessage
| SDKTaskUpdatedMessage | SDKTaskProgressMessage | SDKBackgroundTasksChangedMessage
| SDKThinkingTokensMessage | SDKSessionStateChangedMessage
| SDKWorkerShuttingDownMessage | SDKCommandsChangedMessage | SDKNotificationMessage
| SDKFilesPersistedEvent | SDKToolUseSummaryMessage | SDKMemoryRecallMessage
| SDKRateLimitEvent | SDKElicitationCompleteMessage | SDKPermissionDeniedMessage
| SDKPromptSuggestionMessage | SDKMirrorErrorMessage | SDKInformationalMessage
| SDKConversationResetMessage
```

Discriminant: every variant has `type: '...'`; several share `type: 'system'`
and are further discriminated by a `subtype` field. For a minimal sidecar
consumer, the variants that matter are below. All others are advisory/telemetry
system events that can be safely ignored (or logged) by a first-pass
implementation.

### Important correction vs. the brief's phrasing

The brief asks about `session_start`, `text_delta`, `tool_use`, and
`tool_result` as if they were `SDKMessage` variant names. **They are not.**
None of those four strings appears anywhere as a `type` or `subtype` literal
in the `SDKMessage` union. Here's what actually exists for each:

- **"session_start"** → there is no message literally named this. The
  session-bootstrap message is `SDKSystemMessage` with `type: 'system'`,
  `subtype: 'init'` (see below). (The string `'session_start'` does appear
  once in the file, at sdk.d.ts:1064, but only as one value of an unrelated
  `load_reason` enum for file/context loading — not a message type.)
- **"text_delta"** → this is not an `SDKMessage` variant either. It's an
  Anthropic Messages API streaming-event delta type that shows up *inside*
  `SDKPartialAssistantMessage.event` (`event.delta.type === 'text_delta'`
  for a `content_block_delta` event). The top-level SDKMessage variant for
  streamed deltas is `SDKPartialAssistantMessage` (`type: 'stream_event'`).
- **"tool_use"** → not a top-level `SDKMessage` variant. `tool_use` is a
  content-block type inside `SDKAssistantMessage.message.content` (an
  Anthropic `BetaMessage`, imported from `@anthropic-ai/sdk`). A tool call
  is one content block among possibly several in an assistant message.
- **"tool_result"** → likewise not a top-level variant. `tool_result` is a
  content-block type inside `SDKUserMessage.message.content` (a
  `MessageParam` with role `"user"`) — the SDK/CLI synthesizes a `user`
  message carrying the tool's result and feeds it back to the model.

So: Task 5's message-handling code should switch on `SDKMessage.type` (and
`.subtype` where present) for `SDKSystemMessage` / `SDKPartialAssistantMessage`
/ `SDKAssistantMessage` / `SDKUserMessage` / `SDKResultMessage`, and — for the
assistant/user variants — additionally switch on the nested content block's
`type` field (`text`, `tool_use`, `thinking`, `tool_result`, ...) per the
Anthropic Messages API content-block union.

### SDKSystemMessage (sdk.d.ts:5834-5916) — `type: 'system'`, `subtype: 'init'`

The session-bootstrap/session-start message. Key fields:
```typescript
{
    type: 'system';
    subtype: 'init';
    agents?: string[];
    apiKeySource: ApiKeySource;
    claude_code_version: string;
    cwd: string;
    tools: string[];
    mcp_servers: { name: string; status: string; source?: string }[];
    model: string;
    permissionMode: PermissionMode;
    slash_commands: string[];
    output_style: string;
    skills: string[];
    plugins: { name: string; path: string; version?: string }[];
    uuid: UUID;
    session_id: string;
    // (+ many optional advisory fields: betas, effort, capabilities,
    //   fast_mode_state, view_mode, plugin_errors, etc.)
}
```
`session_id` here is the value to persist if you want to `resume` this
session later (see `Options.resume` below).

### SDKPartialAssistantMessage (sdk.d.ts:5422-5445) — `type: 'stream_event'`

The streamed-delta variant (only emitted when the CLI/SDK is configured for
partial/streaming output):
```typescript
{
    type: 'stream_event';
    event: BetaRawMessageStreamEvent;   // imported from @anthropic-ai/sdk — the
                                          // raw Anthropic Messages streaming event
                                          // (message_start, content_block_start,
                                          // content_block_delta, content_block_stop,
                                          // message_delta, message_stop)
    parent_tool_use_id: string | null;
    uuid: UUID;
    session_id: string;
    ttft_ms?: number;
    user_message_uuid?: string;
    user_message_uuids?: string[];
    resume_reason?: string;
}
```
Text deltas arrive as `event.type === 'content_block_delta'` with
`event.delta.type === 'text_delta'` and `event.delta.text: string`.

### SDKAssistantMessage (sdk.d.ts:3602-3681) — `type: 'assistant'`

```typescript
{
    type: 'assistant';
    message: BetaMessage;   // imported from @anthropic-ai/sdk — content blocks
                             // (text, thinking, tool_use, ...), stop_reason, usage
    parent_tool_use_id: string | null;
    error?: SDKAssistantMessageError;
    uuid: UUID;
    session_id: string;
    request_id?: string;
    // (+ many optional bookkeeping fields: user_message_uuid(s), resume_reason,
    //   resumed_from_incomplete_thinking, supersedes, aborted, subagent_type,
    //   task_description, timestamp, context_usage, usage_report)
}
```
A `tool_use` request from the model surfaces as a content block inside
`message.content` here (block shape: `{ type: 'tool_use', id, name, input }`,
from the Anthropic Messages API — not redeclared in this SDK's own `.d.ts`).

### SDKUserMessage — carries `tool_result`

Same type as described above under "SDKUserMessage". When the SDK/CLI
reports a tool's result back into the conversation, it does so as an
`SDKUserMessage` whose `message.content` contains a
`{ type: 'tool_result', tool_use_id, content, is_error? }` block (Anthropic
Messages API shape). `tool_use_result` on the `SDKUserMessage` itself carries
the SDK's richer, per-tool-typed structured output (see the doc-comment
quoted in the "SDKUserMessage" section above) — prefer that over parsing the
`tool_result` block's string/content for anything beyond raw model-facing
text.

### SDKResultMessage (sdk.d.ts:5669) — `type: 'result'`

`SDKResultMessage = SDKResultSuccess | SDKResultError`, discriminated by
`subtype`:

- `SDKResultSuccess` (sdk.d.ts:5671+): `subtype: 'success'`. Key fields:
  `duration_ms`, `duration_api_ms`, `is_error: boolean`, `num_turns: number`,
  `result: string` (the final assistant text), `stop_reason: string | null`,
  `total_cost_usd: number`, `usage: NonNullableUsage`,
  `modelUsage: Record<string, ModelUsage>`, `permission_denials: SDKPermissionDenial[]`,
  `structured_output?: unknown`, `uuid: UUID`, `session_id: string`.
- `SDKResultError` (sdk.d.ts:5610+): `subtype: 'error_during_execution' |
  'error_max_turns' | 'error_max_budget_usd' | 'error_max_structured_output_retries'`.
  Same bookkeeping fields as success (`duration_ms`, `total_cost_usd`, `usage`,
  `modelUsage`, `permission_denials`, `uuid`, `session_id`, ...) plus
  `errors: string[]` and no `result` field.

Exactly one `result` message is emitted per turn, after that turn's
assistant/user/stream_event messages — treat it as the turn-complete signal.

## CanUseTool (sdk.d.ts:213-301)

```typescript
export declare type CanUseTool = (
    toolName: string,
    input: Record<string, unknown>,
    options: {
        signal: AbortSignal;
        suggestions?: PermissionUpdate[];
        blockedPath?: string;
        mcpServer?: { name: string; source: string };
        decisionReason?: string;
        title?: string;
        displayName?: string;
        description?: string;
        defaultToNo?: boolean;
        suppressAlwaysAllowRule?: boolean;
        toolUseID: string;
        agentID?: string;
        requestId: string;
        matchedAskRule?: { source: string; toolName: string; ruleContent?: string };
    }
) => Promise<PermissionResult | null>;
```

Wired into `Options.canUseTool?: CanUseTool` (sdk.d.ts:1586).

**Important:** returning `null` is a valid but dangerous escape hatch — it
means "I already sent the control_response out-of-band myself." The doc
comment is explicit that this is fail-closed: an *accidental* null means no
response is ever sent and the tool call blocks forever (no timeout). Task 5's
`canUseTool` implementation should never return `null` unless it has actually
sent a response through another channel — normally it should always resolve
to a `PermissionResult`.

### PermissionResult (sdk.d.ts:2504-2516)

```typescript
export declare type PermissionResult =
  | {
      behavior: 'allow';
      updatedInput?: Record<string, unknown>;
      updatedPermissions?: PermissionUpdate[];
      toolUseID?: string;
      decisionClassification?: PermissionDecisionClassification;
    }
  | {
      behavior: 'deny';
      message: string;
      interrupt?: boolean;
      toolUseID?: string;
      decisionClassification?: PermissionDecisionClassification;
    };
```

`message` is **required** on `deny` (shown to the model as the reason the
tool call was refused). `updatedInput` on `allow` lets the host rewrite the
tool's input before it executes.

## Options.resume

```typescript
resume?: string;
```
(sdk.d.ts:2081-2084): **`string`, not `boolean`.** Doc comment: "Session ID
to resume. Loads the conversation history from the specified session." This
directly answers the brief's question — **an arbitrary past session ID can be
passed directly** via `resume: sessionId`, no special boolean flag needed.

Related, separate options (do not confuse with `resume`):
- `continue?: boolean` (sdk.d.ts:1587-1591) — "Continue the most recent
  conversation in the current directory instead of starting a new one."
  Explicitly documented as **mutually exclusive with `resume`**. This is the
  boolean-only option; it always means "most recent," with no session-id
  targeting.
- `sessionId?: string` (sdk.d.ts:2085-2090) — assigns a custom ID to a *new*
  or *forked* session; cannot be combined with `continue`/`resume` unless
  `forkSession` is also set.
- `resumeSessionAt?: string` (sdk.d.ts:2091-2098) and `resumeDropsTurn?:
  string` (sdk.d.ts:2099-2149) — advanced partial/truncating resume, only
  usable together with `resume`. Not needed for a basic "resume this session
  ID" implementation; documented here only so Task 5 doesn't have to
  re-discover them if truncating-resume is ever needed.

**Conclusion for Task 5:** `start()` (or equivalent) can accept an arbitrary
stored session id and pass it straight through as `options.resume`. No
adjustment needed — the original assumption (pass a session id directly) was
correct; it is `continue` that is boolean-only, and it's a different, less
capable option that Task 5 should not use for resuming a specific session.
