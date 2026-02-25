# MCP & ACP Overhaul Plan

## ACP Improvements

### 1. Types alignment (types.ts)
- Add sampling params: `ACPSamplingParams` (temperature, maxTokens, stopSequences, topP, topK)
- Add tool types: `ACPToolCall`, `ACPToolCallUpdate`, `ACPToolResult`
- Add resource types: `ACPResource`, `ACPResourceListResult`
- Add session management types: `ACPSessionListResult`, `ACPSessionLoadResult`
- Add `ACPClientCapabilities` for init advertisement
- Add model selection types from session/new response

### 2. Connection improvements (acp-connection.ts)
- Route stderr to logger instead of ignoring
- Add AbortSignal support for request cancellation
- Add heartbeat/liveness detection
- Improve malformed message logging

### 3. Client improvements (acp-client.ts)
- Multi-turn sessions: session pool, reuse across chain steps
- Sampling params on generate/chat/stream
- Session management: list, load, delete
- Tool call handling: parse tool_call notifications, handle tool results
- Capability advertisement in initialize
- Model selection support
- Request cancellation via AbortSignal

### 4. Results improvements (acp-results.ts)
- Extract tool call info from session/update notifications
- Parse model info from responses

## MCP Improvements

### 5. Client improvements (mcp-client.ts)
- Logging: setLoggingLevel(), onLoggingMessage handler
- List-changed: register handlers for tool/resource/prompt changes
- Completions: complete() method
- Roots: sendRootsListChanged()
- Retry logic with exponential backoff (matching ACP pattern)
- Resource subscriptions: subscribe/unsubscribe
- Timeout configuration per operation

### 6. Server improvements (mcp-server.ts)
- Logging: sendLoggingMessage()
- List-changed: sendToolListChanged(), sendResourceListChanged(), sendPromptListChanged()
- More tools: memory-search, memory-add, chain-status
- Resource templates with URI patterns
- Tool annotations (readOnlyHint, destructiveHint, etc.)
- Dynamic tool registration/unregistration

### 7. Types improvements (mcp types.ts)
- Add logging types
- Add completion types
- Add roots types
- Add subscription types
- Expand MCPToolInfo with annotations

## Docs

### 8. Update CLAUDE.md, README.md, MEMORY.md
- Reflect new architecture
- Document new capabilities
