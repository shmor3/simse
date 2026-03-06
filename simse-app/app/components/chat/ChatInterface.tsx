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
		// Reset textarea height
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
					<div className="flex h-full flex-col items-center justify-center gap-4">
						<SimseLogo size={48} className="text-zinc-700" />
						<p className="text-sm text-zinc-500">What would you like to do?</p>
					</div>
				) : (
					/* Messages + tool calls */
					<div className="py-4 space-y-1">
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
								<span className="inline-block h-4 w-1.5 animate-pulse rounded-full bg-emerald-400" />
							</div>
						)}

						<div ref={messagesEndRef} />
					</div>
				)}
			</div>

			{/* Input area */}
			<div className="shrink-0 border-t border-zinc-800/50 bg-zinc-950 p-4">
				<div className="mx-auto flex max-w-3xl items-end gap-3">
					<textarea
						ref={textareaRef}
						value={input}
						onChange={handleInput}
						onKeyDown={handleKeyDown}
						placeholder="Send a message..."
						rows={1}
						className="flex-1 resize-none rounded-xl border border-zinc-800 bg-zinc-900 px-4 py-3 text-sm text-zinc-100 placeholder-zinc-600 outline-none transition-colors focus:border-zinc-700 focus:ring-2 focus:ring-emerald-400/50"
					/>
					<button
						type="button"
						onClick={handleSend}
						disabled={!input.trim() || isStreaming}
						className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-emerald-400 text-zinc-950 transition-colors hover:bg-emerald-300 active:bg-emerald-500 disabled:pointer-events-none disabled:opacity-50"
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
