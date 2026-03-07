import clsx from 'clsx';
import { useEffect, useRef, useState } from 'react';

interface AcpBackend {
	id: string;
	name: string;
	provider: string;
}

interface AcpSwitcherProps {
	backends: AcpBackend[];
	activeId: string;
	onSelect: (id: string) => void;
}

export default function AcpSwitcher({
	backends,
	activeId,
	onSelect,
}: AcpSwitcherProps) {
	const [open, setOpen] = useState(false);
	const ref = useRef<HTMLDivElement>(null);

	const active = backends.find((b) => b.id === activeId);

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

	return (
		<div ref={ref} className="relative">
			<button
				type="button"
				onClick={() => setOpen((v) => !v)}
				className={clsx(
					'inline-flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm transition-colors',
					'text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200',
				)}
			>
				<span className="font-medium">{active?.name ?? 'Select backend'}</span>
				<svg
					className={clsx(
						'h-3.5 w-3.5 text-zinc-600 transition-transform',
						open && 'rotate-180',
					)}
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					strokeWidth={2}
				>
					<path
						strokeLinecap="round"
						strokeLinejoin="round"
						d="M19 9l-7 7-7-7"
					/>
				</svg>
			</button>

			{open && (
				<div className="absolute left-0 top-full z-50 mt-2 w-64 origin-top-left rounded-xl border border-zinc-800 bg-zinc-900 py-1.5 shadow-2xl animate-scale-in">
					<div className="px-4 py-2">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							ACP Backend
						</p>
					</div>
					<div className="py-1">
						{backends.map((backend) => (
							<button
								key={backend.id}
								type="button"
								onClick={() => {
									onSelect(backend.id);
									setOpen(false);
								}}
								className="flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-zinc-800/50"
							>
								<div className="flex-1 min-w-0">
									<p
										className={clsx(
											'text-sm',
											backend.id === activeId
												? 'text-white font-medium'
												: 'text-zinc-400',
										)}
									>
										{backend.name}
									</p>
									<p className="mt-0.5 text-[12px] text-zinc-600">
										{backend.provider}
									</p>
								</div>
								{backend.id === activeId && (
									<svg
										className="h-4 w-4 shrink-0 text-emerald-400"
										fill="none"
										viewBox="0 0 24 24"
										stroke="currentColor"
										strokeWidth={2.5}
									>
										<path
											strokeLinecap="round"
											strokeLinejoin="round"
											d="M5 13l4 4L19 7"
										/>
									</svg>
								)}
							</button>
						))}
					</div>
				</div>
			)}
		</div>
	);
}
