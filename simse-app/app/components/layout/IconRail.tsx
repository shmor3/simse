import clsx from 'clsx';
import { NavLink } from 'react-router';
import SimseLogo from '../ui/SimseLogo';

interface Remote {
	id: string;
	name: string;
	status: 'connected' | 'offline';
}

interface IconRailProps {
	remotes: Remote[];
	activeId: string | null;
	onSelect: (id: string | null) => void;
}

function initials(name: string): string {
	return name
		.split(/[\s\-_]+/)
		.slice(0, 2)
		.map((w) => w[0])
		.join('')
		.toUpperCase();
}

function Tooltip({ label }: { label: string }) {
	return (
		<div className="pointer-events-none absolute left-full top-1/2 z-50 ml-2.5 -translate-y-1/2 rounded-md bg-zinc-800 px-2.5 py-1 opacity-0 shadow-lg transition-opacity group-hover:opacity-100">
			<span className="whitespace-nowrap text-[12px] font-medium text-zinc-200">
				{label}
			</span>
			{/* Arrow */}
			<div className="absolute -left-1 top-1/2 h-2 w-2 -translate-y-1/2 rotate-45 bg-zinc-800" />
		</div>
	);
}

export default function IconRail({
	remotes,
	activeId,
	onSelect,
}: IconRailProps) {
	return (
		<aside className="flex w-14 flex-col items-center border-r border-zinc-800 bg-zinc-950 py-3">
			{/* Home icon */}
			<div className="group relative">
				{activeId === null && (
					<div className="absolute -left-3 top-1/2 h-5 w-1 -translate-y-1/2 rounded-r-full bg-emerald-400" />
				)}
				<button
					type="button"
					onClick={() => onSelect(null)}
					className={clsx(
						'flex h-10 w-10 items-center justify-center rounded-xl transition-all duration-200',
						activeId === null
							? 'bg-emerald-400/10 text-emerald-400'
							: 'text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-300',
					)}
				>
					<SimseLogo size={20} />
				</button>
				<Tooltip label="Home" />
			</div>

			{/* Divider */}
			{remotes.length > 0 && (
				<div className="mx-auto my-2 h-px w-6 bg-zinc-800" />
			)}

			{/* Remote icons */}
			<div className="flex flex-1 flex-col items-center gap-2 overflow-y-auto">
				{remotes.map((remote) => (
					<div key={remote.id} className="group relative">
						{activeId === remote.id && (
							<div className="absolute -left-3 top-1/2 h-5 w-1 -translate-y-1/2 rounded-r-full bg-emerald-400" />
						)}
						<button
							type="button"
							onClick={() => onSelect(remote.id)}
							className={clsx(
								'relative flex h-10 w-10 items-center justify-center rounded-xl font-mono text-[11px] font-bold transition-all duration-200',
								activeId === remote.id
									? 'bg-emerald-400/10 text-emerald-400 ring-2 ring-emerald-400/50'
									: 'bg-zinc-800/50 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200',
							)}
						>
							{initials(remote.name)}
							{/* Connected status dot */}
							{remote.status === 'connected' && (
								<span className="absolute -bottom-0.5 -right-0.5 h-2.5 w-2.5 rounded-full border-2 border-zinc-950 bg-emerald-400 animate-pulse-dot" />
							)}
						</button>
						<Tooltip label={remote.name} />
					</div>
				))}
			</div>

			{/* Add remote link */}
			<div className="group relative mt-2">
				<NavLink
					to="/dashboard/settings/remotes"
					className="flex h-10 w-10 items-center justify-center rounded-xl text-zinc-600 transition-all duration-200 hover:bg-zinc-800/60 hover:text-zinc-400"
					title="Add remote"
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
							d="M12 4v16m8-8H4"
						/>
					</svg>
				</NavLink>
				<Tooltip label="Add remote" />
			</div>
		</aside>
	);
}
