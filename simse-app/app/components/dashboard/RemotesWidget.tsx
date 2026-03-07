import { Link } from 'react-router';
import Card from '~/components/ui/Card';

interface Remote {
	id: string;
	name: string;
	status: 'connected' | 'offline';
}

interface RemotesWidgetProps {
	remotes: Remote[];
	onConnect: (id: string) => void;
}

export default function RemotesWidget({
	remotes,
	onConnect,
}: RemotesWidgetProps) {
	if (remotes.length === 0) {
		return (
			<Card className="card-hover p-5">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Remote machines
				</p>
				<div className="mt-4 flex flex-col items-center py-3">
					<div className="flex h-10 w-10 items-center justify-center rounded-full bg-zinc-800">
						<svg
							className="h-5 w-5 text-zinc-600"
							fill="none"
							viewBox="0 0 24 24"
							stroke="currentColor"
							strokeWidth={1.5}
						>
							<path
								strokeLinecap="round"
								strokeLinejoin="round"
								d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2"
							/>
						</svg>
					</div>
					<p className="mt-3 text-sm text-zinc-500">No remotes connected</p>
					<Link
						to="/dashboard/settings/remotes"
						className="mt-2 font-mono text-[11px] text-emerald-400 transition-colors hover:text-emerald-300"
					>
						Add your first remote
					</Link>
				</div>
			</Card>
		);
	}

	return (
		<Card className="card-hover p-5">
			<div className="flex items-center justify-between">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Remote machines
				</p>
				<span className="font-mono text-[10px] text-zinc-600">
					{remotes.filter((r) => r.status === 'connected').length} online
				</span>
			</div>
			<div className="mt-4 space-y-2">
				{remotes.map((remote) => (
					<button
						key={remote.id}
						type="button"
						onClick={() => onConnect(remote.id)}
						className="flex w-full items-center gap-3 rounded-lg border border-zinc-800/50 bg-zinc-950/50 px-3.5 py-2.5 transition-all hover:border-zinc-700 hover:bg-zinc-800/30"
					>
						<span
							className={`h-2 w-2 rounded-full ${
								remote.status === 'connected'
									? 'bg-emerald-400 animate-pulse-dot'
									: 'bg-zinc-600'
							}`}
						/>
						<span className="flex-1 text-left text-sm text-zinc-300">
							{remote.name}
						</span>
						<span className="font-mono text-[10px] uppercase tracking-wider text-zinc-600">
							{remote.status === 'connected' ? 'connected' : 'offline'}
						</span>
						<svg
							className="h-3.5 w-3.5 text-zinc-700"
							fill="none"
							viewBox="0 0 24 24"
							stroke="currentColor"
							strokeWidth={2}
						>
							<path
								strokeLinecap="round"
								strokeLinejoin="round"
								d="M9 5l7 7-7 7"
							/>
						</svg>
					</button>
				))}
			</div>
		</Card>
	);
}
