// ---------------------------------------------------------------------------
// Tool Permission Resolver — glob-based permission rules
// ---------------------------------------------------------------------------

import type {
	ToolAnnotations,
	ToolCallRequest,
	ToolCategory,
	ToolDefinition,
	ToolPermissionResolver,
} from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ToolPermissionPolicy = 'allow' | 'ask' | 'deny';

export interface ToolPermissionRule {
	readonly tool: string;
	readonly pattern?: string;
	readonly policy: ToolPermissionPolicy;
	/** Match tools by category. Requires `definition` to be passed to `check()`. */
	readonly category?: ToolCategory | readonly ToolCategory[];
	/** Match tools by annotation values. Requires `definition` to be passed to `check()`. */
	readonly annotations?: Partial<ToolAnnotations>;
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
	const check = async (
		request: ToolCallRequest,
		definition?: ToolDefinition,
	): Promise<boolean> => {
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

			// Category matching: skip rule if definition doesn't have matching category
			if (rule.category !== undefined) {
				if (!definition?.category) continue;
				const cats = Array.isArray(rule.category)
					? rule.category
					: [rule.category];
				if (!cats.includes(definition.category)) continue;
			}

			// Annotation matching: skip rule if definition doesn't match annotations
			if (rule.annotations !== undefined) {
				if (!definition?.annotations) continue;
				let match = true;
				for (const [key, value] of Object.entries(rule.annotations)) {
					if (
						(definition.annotations as Record<string, unknown>)[key] !== value
					) {
						match = false;
						break;
					}
				}
				if (!match) continue;
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
