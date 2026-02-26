// ---------------------------------------------------------------------------
// Tool Permission Resolver — glob-based permission rules
// ---------------------------------------------------------------------------

import type { ToolCallRequest, ToolPermissionResolver } from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ToolPermissionPolicy = 'allow' | 'ask' | 'deny';

export interface ToolPermissionRule {
	readonly tool: string;
	readonly pattern?: string;
	readonly policy: ToolPermissionPolicy;
}

export interface ToolPermissionConfig {
	readonly defaultPolicy: ToolPermissionPolicy;
	readonly rules: readonly ToolPermissionRule[];
	readonly onPermissionRequest?: (request: ToolCallRequest) => Promise<boolean>;
}

// ---------------------------------------------------------------------------
// Glob → RegExp
// ---------------------------------------------------------------------------

function globToRegExp(glob: string): RegExp {
	let escaped = '';
	for (const ch of glob) {
		if (ch === '*') {
			escaped += '.*';
		} else if (ch === '?') {
			escaped += '.';
		} else if (/[.+^${}()|[\]\\]/.test(ch)) {
			escaped += `\\${ch}`;
		} else {
			escaped += ch;
		}
	}
	return new RegExp(`^${escaped}$`);
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createToolPermissionResolver(
	config: ToolPermissionConfig,
): ToolPermissionResolver {
	const check = async (request: ToolCallRequest): Promise<boolean> => {
		let resolved: ToolPermissionPolicy = config.defaultPolicy;

		for (const rule of config.rules) {
			const toolRe = globToRegExp(rule.tool);
			if (!toolRe.test(request.name)) continue;

			// If the rule has a command pattern, only match when the tool is
			// a bash-like tool and the command argument matches.
			if (rule.pattern !== undefined) {
				const command = request.arguments.command;
				if (typeof command !== 'string') continue;
				const cmdRe = globToRegExp(rule.pattern);
				if (!cmdRe.test(command)) continue;
			}

			resolved = rule.policy;
		}

		if (resolved === 'allow') return true;
		if (resolved === 'deny') return false;

		// ask
		if (config.onPermissionRequest) {
			return config.onPermissionRequest(request);
		}
		return false;
	};

	return Object.freeze({ check });
}
