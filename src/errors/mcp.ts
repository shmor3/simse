// ---------------------------------------------------------------------------
// MCP Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createMCPError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		statusCode?: number;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createSimseError(message, {
		name: options.name ?? 'MCPError',
		code: options.code ?? 'MCP_ERROR',
		statusCode: options.statusCode ?? 500,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createMCPConnectionError = (
	serverName: string,
	message: string,
	options: { cause?: unknown } = {},
): SimseError & { readonly serverName: string } => {
	const err = createMCPError(`MCP server "${serverName}": ${message}`, {
		name: 'MCPConnectionError',
		code: 'MCP_CONNECTION_ERROR',
		statusCode: 503,
		cause: options.cause,
		metadata: { serverName },
	}) as SimseError & { readonly serverName: string };

	Object.defineProperty(err, 'serverName', {
		value: serverName,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createMCPServerNotConnectedError = (
	serverName: string,
): SimseError & { readonly serverName: string } => {
	const err = createMCPError(
		`MCP server "${serverName}" is not connected. Call connect("${serverName}") first.`,
		{
			name: 'MCPServerNotConnectedError',
			code: 'MCP_NOT_CONNECTED',
			statusCode: 503,
			metadata: { serverName },
		},
	) as SimseError & { readonly serverName: string };

	Object.defineProperty(err, 'serverName', {
		value: serverName,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createMCPToolError = (
	serverName: string,
	toolName: string,
	message: string,
	options: { cause?: unknown } = {},
): SimseError & { readonly serverName: string; readonly toolName: string } => {
	const err = createMCPError(
		`MCP tool "${toolName}" on server "${serverName}": ${message}`,
		{
			name: 'MCPToolError',
			code: 'MCP_TOOL_ERROR',
			statusCode: 502,
			cause: options.cause,
			metadata: { serverName, toolName },
		},
	) as SimseError & { readonly serverName: string; readonly toolName: string };

	Object.defineProperties(err, {
		serverName: { value: serverName, writable: false, enumerable: true },
		toolName: { value: toolName, writable: false, enumerable: true },
	});

	return err;
};

export const createMCPTransportConfigError = (
	serverName: string,
	message: string,
): SimseError & { readonly serverName: string } => {
	const err = createMCPError(`MCP server "${serverName}": ${message}`, {
		name: 'MCPTransportConfigError',
		code: 'MCP_TRANSPORT_CONFIG',
		statusCode: 400,
		metadata: { serverName },
	}) as SimseError & { readonly serverName: string };

	Object.defineProperty(err, 'serverName', {
		value: serverName,
		writable: false,
		enumerable: true,
	});

	return err;
};

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isMCPError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code.startsWith('MCP_');

export const isMCPConnectionError = (
	value: unknown,
): value is SimseError & { readonly serverName: string } =>
	isSimseError(value) && value.code === 'MCP_CONNECTION_ERROR';

export const isMCPServerNotConnectedError = (
	value: unknown,
): value is SimseError & { readonly serverName: string } =>
	isSimseError(value) && value.code === 'MCP_NOT_CONNECTED';

export const isMCPToolError = (
	value: unknown,
): value is SimseError & {
	readonly serverName: string;
	readonly toolName: string;
} => isSimseError(value) && value.code === 'MCP_TOOL_ERROR';

export const isMCPTransportConfigError = (
	value: unknown,
): value is SimseError & { readonly serverName: string } =>
	isSimseError(value) && value.code === 'MCP_TRANSPORT_CONFIG';
