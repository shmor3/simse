import { useCallback, useEffect, useRef, useState } from 'react';
import SimseLogo from '../ui/SimseLogo';
import MessageBubble from './MessageBubble';
import ToolCallCard from './ToolCallCard';

interface Message {
	id: string;
	role: 'user' | 'assistant' | 'system';
	content: string;
}

interface ToolCall {
	id: string;
	name: string;
	input: string;
	output?: string;
	status: 'running' | 'completed' | 'error';
}

interface ChatInterfaceProps {
	messages: Message[];
	toolCalls: ToolCall[];
	onSend: (message: string) => void;
	isStreaming?: boolean;
}

export default function ChatInterface({
	messages,
	toolCalls,
	onSend,
	isStreaming = false,
}: ChatInterfaceProps) {
	const [input, setInput] = useState('');
	const messagesEndRef = useRef<HTMLDivElement>(null);
	const textareaRef = useRef<HTMLTextAreaElement>(null);

	// Auto-scroll to bottom on new messages
	// biome-ignore lint/correctness/useExhaustiveDependencies: scroll on count change
	useEffect(() => {
		messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
	}, [messages.length, toolCalls.length]);

	// Auto-resize textarea
	const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
		setInput(e.target.value);
		const el = e.target;
		el.style.height = 'auto';
		el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
	};

	const handleSend = useCallback(() => {
		const trimmed = input.trim();
		if (!trimmed || isStreaming) return;
		onSend(trimmed);
		setInput('');
		if (textareaRef.current) {
			textareaRef.current.style.height = 'auto';
		}
	}, [input, isStreaming, onSend]);

	const handleKeyDown = useCallback(
		(e: React.KeyboardEvent<HTMLTextAreaElement>) => {
			if (e.key === 'Enter' && !e.shiftKey) {
				e.preventDefault();
				handleSend();
			}
		},
		[handleSend],
	);

	const isEmpty = messages.length === 0;

	return (
		<div className="flex h-full flex-col">
			{/* Message area */}
			<div className="flex-1 overflow-y-auto">
				{isEmpty ? (
					/* Empty state */
					<div className="flex h-full flex-col items-center justify-center gap-5 animate-fade-in">
						<div className="relative">
							<div className="absolute inset-0 rounded-full bg-emerald-400/5 blur-2xl" />
							<SimseLogo size={56} className="relative text-zinc-700" />
						</div>
						<div className="text-center">
							<p className="text-sm font-medium text-zinc-400">
								What would you like to do?
							</p>
							<p className="mt-1 text-[13px] text-zinc-600">
								Type a message to get started.
							</p>
						</div>
						{/* Suggestions */}
						<div className="mt-2 flex flex-wrap justify-center gap-2">
							{[
								'Explain this codebase',
								'Find and fix bugs',
								'Write tests',
							].map((suggestion) => (
								<button
									key={suggestion}
									type="button"
									onClick={() => {
										setInput(suggestion);
										textareaRef.current?.focus();
									}}
									className="rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-1.5 text-[13px] text-zinc-500 transition-colors hover:border-zinc-700 hover:text-zinc-300"
								>
									{suggestion}
								</button>
							))}
						</div>
					</div>
				) : (
					/* Messages + tool calls */
					<div className="space-y-1 py-4">
						{messages.map((msg) => (
							<MessageBubble
								key={msg.id}
								role={msg.role}
								content={msg.content}
							/>
						))}

						{/* Tool calls */}
						{toolCalls.map((tc) => (
							<ToolCallCard
								key={tc.id}
								name={tc.name}
								input={tc.input}
								output={tc.output}
								status={tc.status}
							/>
						))}

						{/* Streaming indicator */}
						{isStreaming && (
							<div className="mx-auto max-w-3xl px-4 py-2">
								<div className="flex items-center gap-1.5">
									<span
										className="h-1.5 w-1.5 rounded-full bg-emerald-400"
										style={{ animation: 'blink 1.4s infinite 0s' }}
									/>
									<span
										className="h-1.5 w-1.5 rounded-full bg-emerald-400"
										style={{ animation: 'blink 1.4s infinite 0.2s' }}
									/>
									<span
										className="h-1.5 w-1.5 rounded-full bg-emerald-400"
										style={{ animation: 'blink 1.4s infinite 0.4s' }}
									/>
								</div>
							</div>
						)}

						<div ref={messagesEndRef} />
					</div>
				)}
			</div>

			{/* Input area */}
			<div className="shrink-0 border-t border-zinc-800/50 bg-zinc-950 p-4">
				<div className="mx-auto flex max-w-3xl items-end gap-3">
					<div className="relative flex-1">
						<textarea
							ref={textareaRef}
							value={input}
							onChange={handleInput}
							onKeyDown={handleKeyDown}
							placeholder="Send a message..."
							rows={1}
							className="w-full resize-none rounded-xl border border-zinc-800 bg-zinc-900 px-4 py-3 pr-10 text-sm text-zinc-100 placeholder-zinc-600 outline-none transition-colors focus:border-zinc-700 focus:ring-2 focus:ring-emerald-400/20"
						/>
						<div className="pointer-events-none absolute bottom-3 right-3 font-mono text-[10px] text-zinc-700">
							<kbd>Enter</kbd>
						</div>
					</div>
					<button
						type="button"
						onClick={handleSend}
						disabled={!input.trim() || isStreaming}
						className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-emerald-400 text-zinc-950 transition-all hover:bg-emerald-300 hover:shadow-lg hover:shadow-emerald-400/10 active:bg-emerald-500 disabled:pointer-events-none disabled:opacity-50"
					>
						<svg
							className="h-5 w-5"
							fill="none"
							viewBox="0 0 24 24"
							stroke="currentColor"
							strokeWidth={2}
						>
							<path
								strokeLinecap="round"
								strokeLinejoin="round"
								d="M5 12h14M12 5l7 7-7 7"
							/>
						</svg>
					</button>
				</div>
			</div>
		</div>
	);
}
