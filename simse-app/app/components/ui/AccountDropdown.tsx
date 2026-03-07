import { useEffect, useRef, useState } from 'react';
import { Form, Link } from 'react-router';
import Avatar from './Avatar';

interface AccountDropdownProps {
	name: string;
	email: string;
}

export default function AccountDropdown({ name, email }: AccountDropdownProps) {
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

	return (
		<div ref={ref} className="relative">
			<button
				type="button"
				onClick={() => setOpen((v) => !v)}
				className="flex items-center gap-2.5 rounded-lg px-2 py-1.5 transition-colors hover:bg-zinc-800/60"
			>
				<Avatar name={name} size="sm" />
				<span className="hidden text-sm text-zinc-400 sm:block">{name}</span>
				<svg
					className="h-3.5 w-3.5 text-zinc-600"
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
				<div className="absolute right-0 top-full z-50 mt-2 w-64 origin-top-right rounded-xl border border-zinc-800 bg-zinc-900 py-1.5 shadow-2xl animate-scale-in">
					{/* User info */}
					<div className="px-4 py-3">
						<p className="text-sm font-medium text-white">{name}</p>
						<p className="mt-0.5 text-[13px] text-zinc-500">{email}</p>
					</div>
					<div className="mx-3 border-t border-zinc-800" />

					{/* Menu items */}
					<div className="py-1.5">
						<Link
							to="/dashboard/account"
							onClick={() => setOpen(false)}
							className="flex items-center gap-3 px-4 py-2 text-sm text-zinc-400 transition-colors hover:bg-zinc-800/50 hover:text-zinc-200"
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
									d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
								/>
							</svg>
							Account
						</Link>
						<Link
							to="/dashboard/settings"
							onClick={() => setOpen(false)}
							className="flex items-center gap-3 px-4 py-2 text-sm text-zinc-400 transition-colors hover:bg-zinc-800/50 hover:text-zinc-200"
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
									d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
								/>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
								/>
							</svg>
							Settings
						</Link>
						<a
							href="https://simse.dev/docs"
							target="_blank"
							rel="noopener noreferrer"
							onClick={() => setOpen(false)}
							className="flex items-center gap-3 px-4 py-2 text-sm text-zinc-400 transition-colors hover:bg-zinc-800/50 hover:text-zinc-200"
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
									d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
								/>
							</svg>
							Help
							<svg
								className="ml-auto h-3 w-3 text-zinc-700"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={2}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
								/>
							</svg>
						</a>
					</div>
					<div className="mx-3 border-t border-zinc-800" />

					{/* Sign out */}
					<div className="py-1.5">
						<Form method="post" action="/auth/logout">
							<button
								type="submit"
								className="flex w-full items-center gap-3 px-4 py-2 text-sm text-zinc-400 transition-colors hover:bg-red-500/10 hover:text-red-400"
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
										d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"
									/>
								</svg>
								Sign out
							</button>
						</Form>
					</div>
				</div>
			)}
		</div>
	);
}
