// ---------------------------------------------------------------------------
// Hook System — Plugin lifecycle hooks for tool/prompt interception
// ---------------------------------------------------------------------------

import type {
	BlockedResult,
	HookContextMap,
	HookHandler,
	HookResultMap,
	HookSystem,
	HookType,
} from './types.js';

// ---------------------------------------------------------------------------
// BlockedResult guard
// ---------------------------------------------------------------------------

function isBlocked(value: unknown): value is BlockedResult {
	return (
		typeof value === 'object' &&
		value !== null &&
		'blocked' in value &&
		(value as BlockedResult).blocked === true
	);
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createHookSystem(): HookSystem {
	const handlers = new Map<HookType, Set<HookHandler<HookType>>>();

	function getSet<T extends HookType>(type: T): Set<HookHandler<T>> {
		let set = handlers.get(type);
		if (!set) {
			set = new Set();
			handlers.set(type, set);
		}
		return set as unknown as Set<HookHandler<T>>;
	}

	function register<T extends HookType>(
		type: T,
		handler: HookHandler<T>,
	): () => void {
		const set = getSet(type);
		set.add(handler);
		return () => {
			set.delete(handler);
		};
	}

	async function run<T extends HookType>(
		type: T,
		context: HookContextMap[T],
	): Promise<HookResultMap[T]> {
		const set = handlers.get(type) as unknown as
			| Set<HookHandler<T>>
			| undefined;
		const hookList = set ? [...set] : [];

		switch (type) {
			case 'tool.execute.before':
				return runBefore(
					hookList as unknown as HookHandler<'tool.execute.before'>[],
					context as HookContextMap['tool.execute.before'],
				) as Promise<HookResultMap[T]>;

			case 'tool.execute.after':
				return runAfter(
					hookList as unknown as HookHandler<'tool.execute.after'>[],
					context as HookContextMap['tool.execute.after'],
				) as Promise<HookResultMap[T]>;

			case 'tool.result.validate':
				return runValidate(
					hookList as unknown as HookHandler<'tool.result.validate'>[],
					context as HookContextMap['tool.result.validate'],
				) as Promise<HookResultMap[T]>;

			case 'prompt.system.transform':
				return runPromptTransform(
					hookList as unknown as HookHandler<'prompt.system.transform'>[],
					context as HookContextMap['prompt.system.transform'],
				) as Promise<HookResultMap[T]>;

			case 'prompt.messages.transform':
				return runMessagesTransform(
					hookList as unknown as HookHandler<'prompt.messages.transform'>[],
					context as HookContextMap['prompt.messages.transform'],
				) as Promise<HookResultMap[T]>;

			case 'session.compacting':
				return runCompacting(
					hookList as unknown as HookHandler<'session.compacting'>[],
					context as HookContextMap['session.compacting'],
				) as Promise<HookResultMap[T]>;

			default: {
				const _exhaustive: never = type;
				throw new Error(`Unknown hook type: ${_exhaustive}`);
			}
		}
	}

	return Object.freeze({ register, run });
}

// ---------------------------------------------------------------------------
// Per-type runners
// ---------------------------------------------------------------------------

async function runBefore(
	hooks: readonly HookHandler<'tool.execute.before'>[],
	context: HookContextMap['tool.execute.before'],
): Promise<HookResultMap['tool.execute.before']> {
	let current = context.request;

	for (const hook of hooks) {
		const result = await hook({ request: current });
		if (isBlocked(result)) {
			return result;
		}
		current = result;
	}

	return current;
}

async function runAfter(
	hooks: readonly HookHandler<'tool.execute.after'>[],
	context: HookContextMap['tool.execute.after'],
): Promise<HookResultMap['tool.execute.after']> {
	let current = context.result;

	for (const hook of hooks) {
		current = await hook({ request: context.request, result: current });
	}

	return current;
}

async function runValidate(
	hooks: readonly HookHandler<'tool.result.validate'>[],
	context: HookContextMap['tool.result.validate'],
): Promise<HookResultMap['tool.result.validate']> {
	const messages: string[] = [];

	for (const hook of hooks) {
		const result = await hook(context);
		messages.push(...result);
	}

	return messages;
}

async function runPromptTransform(
	hooks: readonly HookHandler<'prompt.system.transform'>[],
	context: HookContextMap['prompt.system.transform'],
): Promise<HookResultMap['prompt.system.transform']> {
	let current = context.prompt;

	for (const hook of hooks) {
		current = await hook({ prompt: current });
	}

	return current;
}

async function runMessagesTransform(
	hooks: readonly HookHandler<'prompt.messages.transform'>[],
	context: HookContextMap['prompt.messages.transform'],
): Promise<HookResultMap['prompt.messages.transform']> {
	let current = context.messages;

	for (const hook of hooks) {
		current = await hook({ messages: current });
	}

	return current;
}

async function runCompacting(
	hooks: readonly HookHandler<'session.compacting'>[],
	context: HookContextMap['session.compacting'],
): Promise<HookResultMap['session.compacting']> {
	let current = context.summary;

	for (const hook of hooks) {
		current = await hook({ messages: context.messages, summary: current });
	}

	return current;
}
