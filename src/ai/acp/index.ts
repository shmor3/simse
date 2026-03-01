// ---------------------------------------------------------------------------
// ACP — barrel re-export
// ---------------------------------------------------------------------------

export type {
	ACPEmbedderOptions,
	ACPGeneratorOptions,
} from './acp-adapters.js';
export { createACPEmbedder, createACPGenerator } from './acp-adapters.js';
export type {
	ACPClient,
	ACPClientOptions,
	ACPPermissionOption,
	ACPPermissionRequestInfo,
	ACPPermissionToolCall,
	ACPStreamOptions,
} from './acp-client.js';
export { createACPClient } from './acp-client.js';
export type {
	AcpEngineClient,
	AcpEngineClientOptions,
} from './acp-engine-client.js';
export { createAcpEngineClient } from './acp-engine-client.js';

export type {
	ACPAgentCapabilities,
	ACPAgentInfo,
	ACPChatMessage,
	ACPChatOptions,
	ACPClientCapabilities,
	ACPConfig,
	ACPContentBlock,
	ACPDataContent,
	ACPEmbedResult,
	ACPGenerateOptions,
	ACPGenerateResult,
	ACPInitializeResult,
	ACPMCPServerConfig,
	ACPModeInfo,
	ACPModelInfo,
	ACPModelsInfo,
	ACPModesInfo,
	ACPPermissionPolicy,
	ACPResourceContent,
	ACPResourceLinkContent,
	ACPSamplingParams,
	ACPServerEntry,
	ACPServerInfo,
	ACPServerStatus,
	ACPSessionInfo,
	ACPSessionListEntry,
	ACPSessionPromptResult,
	ACPStopReason,
	ACPStreamChunk,
	ACPStreamComplete,
	ACPStreamDelta,
	ACPStreamToolCall,
	ACPStreamToolCallUpdate,
	ACPTextContent,
	ACPTokenUsage,
	ACPToolCall,
	ACPToolCallUpdate,
	JsonRpcError,
	JsonRpcMessage,
	JsonRpcNotification,
	JsonRpcRequest,
	JsonRpcResponse,
} from './types.js';
