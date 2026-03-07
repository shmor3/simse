import clsx from 'clsx';
import { useState } from 'react';
import PageHeader from '~/components/layout/PageHeader';
import Badge from '~/components/ui/Badge';
import Card from '~/components/ui/Card';
import EmptyState from '~/components/ui/EmptyState';
import type { Route } from './+types/dashboard.library';

interface LibraryItem {
	id: string;
	title: string;
	snippet: string;
	topics: string[];
	similarity?: number;
	addedAt: string;
}

export async function loader(_args: Route.LoaderArgs) {
	return {
		items: [
			{
				id: '1',
				title: 'API rate limiting patterns',
				snippet:
					'Token bucket and sliding window algorithms for rate limiting HTTP APIs. Includes Redis-based distributed implementations...',
				topics: ['architecture', 'api'],
				similarity: 0.94,
				addedAt: '2 days ago',
			},
			{
				id: '2',
				title: 'Rust error handling conventions',
				snippet:
					'Consistent error enum patterns with thiserror. Domain-specific error codes, conversion traits, and JSON-RPC error mapping...',
				topics: ['rust', 'patterns'],
				similarity: 0.91,
				addedAt: '3 days ago',
			},
			{
				id: '3',
				title: 'Database migration strategies',
				snippet:
					'Zero-downtime migration patterns for PostgreSQL and D1. Expand-contract pattern, backfill strategies, and rollback procedures...',
				topics: ['database', 'devops'],
				similarity: 0.87,
				addedAt: '5 days ago',
			},
			{
				id: '4',
				title: 'WebSocket reconnection logic',
				snippet:
					'Exponential backoff with jitter for WebSocket reconnection. Handles token refresh, message queue draining, and session resumption...',
				topics: ['networking', 'realtime'],
				similarity: 0.85,
				addedAt: '1 week ago',
			},
			{
				id: '5',
				title: 'React Router v7 data loading',
				snippet:
					'Loader/action patterns with type-safe route params. Nested layouts, error boundaries, and optimistic UI patterns...',
				topics: ['react', 'frontend'],
				similarity: 0.82,
				addedAt: '1 week ago',
			},
			{
				id: '6',
				title: 'Cloudflare Worker security headers',
				snippet:
					'CSP, CORS, and security header middleware for Hono on Cloudflare Workers. Includes nonce generation and report-uri configuration...',
				topics: ['security', 'cloudflare'],
				similarity: 0.79,
				addedAt: '2 weeks ago',
			},
		] as LibraryItem[],
		allTopics: [
			'architecture',
			'api',
			'rust',
			'patterns',
			'database',
			'devops',
			'networking',
			'realtime',
			'react',
			'frontend',
			'security',
			'cloudflare',
		],
	};
}

type ViewMode = 'grid' | 'list';

export default function Library({ loaderData }: Route.ComponentProps) {
	const { items, allTopics } = loaderData;
	const [search, setSearch] = useState('');
	const [view, setView] = useState<ViewMode>('grid');
	const [activeTopic, setActiveTopic] = useState<string | null>(null);

	const filtered = items.filter((item) => {
		const matchesSearch =
			!search ||
			item.title.toLowerCase().includes(search.toLowerCase()) ||
			item.snippet.toLowerCase().includes(search.toLowerCase());
		const matchesTopic = !activeTopic || item.topics.includes(activeTopic);
		return matchesSearch && matchesTopic;
	});

	return (
		<>
			<PageHeader
				title="Library"
				description="Your knowledge base — search, browse, and manage saved items."
			/>

			{/* Search + filters */}
			<div className="mt-8 animate-fade-in-up">
				<div className="flex items-center gap-3">
					{/* Search input */}
					<div className="relative flex-1">
						<svg
							className="absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-600"
							fill="none"
							viewBox="0 0 24 24"
							stroke="currentColor"
							strokeWidth={2}
						>
							<path
								strokeLinecap="round"
								strokeLinejoin="round"
								d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
							/>
						</svg>
						<input
							type="text"
							placeholder="Search library..."
							value={search}
							onChange={(e) => setSearch(e.target.value)}
							className="w-full rounded-lg border border-zinc-800 bg-zinc-900 py-2.5 pl-10 pr-4 text-sm text-zinc-200 placeholder-zinc-600 transition-colors focus:border-emerald-400/50 focus:outline-none focus:ring-1 focus:ring-emerald-400/20"
						/>
					</div>

					{/* View toggle */}
					<div className="flex gap-1 rounded-lg border border-zinc-800 bg-zinc-900 p-0.5">
						<button
							type="button"
							onClick={() => setView('grid')}
							className={clsx(
								'rounded-md p-2 transition-colors',
								view === 'grid'
									? 'bg-zinc-800 text-white'
									: 'text-zinc-500 hover:text-zinc-300',
							)}
							title="Grid view"
						>
							<svg
								className="h-4 w-4"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={2}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zm0 10a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zm10-10a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zm0 10a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z"
								/>
							</svg>
						</button>
						<button
							type="button"
							onClick={() => setView('list')}
							className={clsx(
								'rounded-md p-2 transition-colors',
								view === 'list'
									? 'bg-zinc-800 text-white'
									: 'text-zinc-500 hover:text-zinc-300',
							)}
							title="List view"
						>
							<svg
								className="h-4 w-4"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={2}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M4 6h16M4 12h16M4 18h16"
								/>
							</svg>
						</button>
					</div>
				</div>

				{/* Topic filters */}
				<div className="mt-3 flex flex-wrap gap-1.5">
					<button
						type="button"
						onClick={() => setActiveTopic(null)}
						className={clsx(
							'rounded-md px-2.5 py-1 font-mono text-[10px] font-bold uppercase tracking-wider transition-colors',
							!activeTopic
								? 'bg-emerald-400/10 text-emerald-400'
								: 'bg-zinc-800/50 text-zinc-500 hover:text-zinc-300',
						)}
					>
						All
					</button>
					{allTopics.map((topic) => (
						<button
							key={topic}
							type="button"
							onClick={() =>
								setActiveTopic(activeTopic === topic ? null : topic)
							}
							className={clsx(
								'rounded-md px-2.5 py-1 font-mono text-[10px] font-bold uppercase tracking-wider transition-colors',
								activeTopic === topic
									? 'bg-emerald-400/10 text-emerald-400'
									: 'bg-zinc-800/50 text-zinc-500 hover:text-zinc-300',
							)}
						>
							{topic}
						</button>
					))}
				</div>
			</div>

			{/* Results */}
			{filtered.length === 0 ? (
				<div className="mt-8">
					<EmptyState
						type="library"
						title="No items found"
						description={
							search || activeTopic
								? 'Try adjusting your search or filters.'
								: 'Add your first item to start building your knowledge base.'
						}
						actionLabel={!search && !activeTopic ? 'Add first item' : undefined}
						actionTo={!search && !activeTopic ? '#' : undefined}
					/>
				</div>
			) : view === 'grid' ? (
				<div className="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
					{filtered.map((item, i) => (
						<div
							key={item.id}
							className={`animate-stagger-${Math.min(i + 1, 6)}`}
						>
							<Card className="card-hover flex h-full flex-col p-5">
								<div className="flex items-start justify-between gap-2">
									<h3 className="text-sm font-semibold text-white">
										{item.title}
									</h3>
									{item.similarity && (
										<span className="shrink-0 font-mono text-[10px] text-emerald-400">
											{(item.similarity * 100).toFixed(0)}%
										</span>
									)}
								</div>
								<p className="mt-2 flex-1 text-[13px] leading-relaxed text-zinc-500">
									{item.snippet}
								</p>
								<div className="mt-4 flex items-center justify-between">
									<div className="flex gap-1.5">
										{item.topics.map((t) => (
											<Badge key={t} variant="default">
												{t}
											</Badge>
										))}
									</div>
									<span className="font-mono text-[10px] text-zinc-700">
										{item.addedAt}
									</span>
								</div>
							</Card>
						</div>
					))}
				</div>
			) : (
				<Card className="mt-6 overflow-hidden animate-fade-in-up">
					<div className="divide-y divide-zinc-800/50">
						{filtered.map((item) => (
							<div
								key={item.id}
								className="flex items-center gap-5 px-6 py-4 transition-colors hover:bg-zinc-800/20"
							>
								<div className="min-w-0 flex-1">
									<div className="flex items-center gap-3">
										<h3 className="text-sm font-semibold text-white">
											{item.title}
										</h3>
										<div className="flex gap-1.5">
											{item.topics.map((t) => (
												<Badge key={t} variant="default">
													{t}
												</Badge>
											))}
										</div>
									</div>
									<p className="mt-1 text-[13px] text-zinc-500 line-clamp-1">
										{item.snippet}
									</p>
								</div>
								{item.similarity && (
									<span className="shrink-0 font-mono text-[11px] text-emerald-400">
										{(item.similarity * 100).toFixed(0)}%
									</span>
								)}
								<span className="shrink-0 font-mono text-[10px] text-zinc-700">
									{item.addedAt}
								</span>
							</div>
						))}
					</div>
				</Card>
			)}

			{/* Count */}
			{filtered.length > 0 && (
				<p className="mt-4 text-center font-mono text-[11px] text-zinc-600">
					{filtered.length} item{filtered.length !== 1 ? 's' : ''}
					{activeTopic ? ` in ${activeTopic}` : ''}
				</p>
			)}
		</>
	);
}
