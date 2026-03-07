import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router';

interface Notification {
	id: string;
	type: string;
	title: string;
	body: string;
	read: boolean;
	created_at: string;
}

interface NotificationsBellProps {
	unreadCount: number;
	notifications: Notification[];
}

function relativeTime(dateStr: string): string {
	const now = Date.now();
	const then = new Date(dateStr).getTime();
	const diff = Math.max(0, now - then);
	const mins = Math.floor(diff / 60000);
	if (mins < 1) return 'just now';
	if (mins < 60) return `${mins}m ago`;
	const hrs = Math.floor(mins / 60);
	if (hrs < 24) return `${hrs}h ago`;
	const days = Math.floor(hrs / 24);
	if (days < 7) return `${days}d ago`;
	return new Date(dateStr).toLocaleDateString();
}

const typeColors: Record<string, string> = {
	success: 'bg-emerald-400',
	warning: 'bg-amber-400',
	billing: 'bg-blue-400',
	info: 'bg-zinc-400',
};

export default function NotificationsBell({
	unreadCount,
	notifications,
}: NotificationsBellProps) {
	const [open, setOpen] = useState(false);
	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (!open) return;
		function onClick(e: MouseEvent) {
			if (ref.current && !ref.current.contains(e.target as Node)) {
				setOpen(false);
			}
		}
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape') setOpen(false);
		}
		document.addEventListener('mousedown', onClick);
		document.addEventListener('keydown', onKey);
		return () => {
			document.removeEventListener('mousedown', onClick);
			document.removeEventListener('keydown', onKey);
		};
	}, [open]);

	const recent = notifications.slice(0, 5);

	return (
		<div ref={ref} className="relative">
			<button
				type="button"
				onClick={() => setOpen((v) => !v)}
				className="relative rounded-lg p-2 text-zinc-500 transition-colors hover:bg-zinc-800/60 hover:text-zinc-300"
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
						d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"
					/>
				</svg>
				{unreadCount > 0 && (
					<span className="absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-emerald-400 px-1 font-mono text-[9px] font-bold text-black">
						{unreadCount > 9 ? '9+' : unreadCount}
					</span>
				)}
			</button>

			{open && (
				<div className="absolute right-0 top-full z-50 mt-2 w-80 origin-top-right rounded-xl border border-zinc-800 bg-zinc-900 shadow-2xl animate-scale-in">
					<div className="flex items-center justify-between border-b border-zinc-800 px-4 py-3">
						<p className="text-sm font-medium text-white">Notifications</p>
						{unreadCount > 0 && (
							<span className="rounded-full bg-emerald-400/15 px-2 py-0.5 font-mono text-[10px] font-bold text-emerald-400">
								{unreadCount} new
							</span>
						)}
					</div>

					{recent.length === 0 ? (
						<div className="px-4 py-8 text-center">
							<svg
								className="mx-auto h-8 w-8 text-zinc-700"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={1.5}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"
								/>
							</svg>
							<p className="mt-2 text-sm text-zinc-500">All caught up</p>
							<p className="mt-0.5 text-[12px] text-zinc-700">
								No new notifications
							</p>
						</div>
					) : (
						<div className="max-h-72 overflow-y-auto divide-y divide-zinc-800/50">
							{recent.map((n) => (
								<div
									key={n.id}
									className={`px-4 py-3 transition-colors hover:bg-zinc-800/30 ${!n.read ? 'bg-zinc-800/20' : ''}`}
								>
									<div className="flex items-start gap-3">
										<span
											className={`mt-1.5 h-2 w-2 shrink-0 rounded-full ${typeColors[n.type] ?? typeColors.info}`}
										/>
										<div className="min-w-0 flex-1">
											<div className="flex items-center justify-between gap-2">
												<p className="text-sm text-white truncate">{n.title}</p>
												<span className="shrink-0 font-mono text-[10px] text-zinc-600">
													{relativeTime(n.created_at)}
												</span>
											</div>
											<p className="mt-0.5 text-[12px] text-zinc-500 line-clamp-1">
												{n.body}
											</p>
										</div>
									</div>
								</div>
							))}
						</div>
					)}

					<div className="border-t border-zinc-800 px-4 py-2.5">
						<Link
							to="/dashboard/notifications"
							onClick={() => setOpen(false)}
							className="block text-center text-[13px] text-zinc-400 transition-colors hover:text-emerald-400"
						>
							View all notifications
						</Link>
					</div>
				</div>
			)}
		</div>
	);
}
