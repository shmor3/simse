import { useState } from 'react';
import type { FormEvent } from 'react';

type FormState = 'idle' | 'loading' | 'success' | 'error';

export default function WaitlistForm() {
	const [email, setEmail] = useState('');
	const [state, setState] = useState<FormState>('idle');
	const [errorMsg, setErrorMsg] = useState('');

	async function handleSubmit(e: FormEvent) {
		e.preventDefault();
		if (!email.trim()) return;

		setState('loading');
		setErrorMsg('');

		try {
			const res = await fetch('/api/waitlist', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ email: email.trim() }),
			});

			if (!res.ok) {
				const data = await res.json().catch(() => ({ error: 'Request failed' }));
				throw new Error(
					(data as { error?: string }).error || `HTTP ${res.status}`,
				);
			}

			setState('success');
		} catch (err) {
			setState('error');
			setErrorMsg(
				err instanceof Error ? err.message : 'Something went wrong',
			);
		}
	}

	if (state === 'success') {
		return (
			<div className="animate-fade-in flex items-center gap-3 rounded-lg border border-emerald-500/30 bg-emerald-500/5 px-6 py-4 font-mono text-sm text-emerald-400">
				<svg
					className="size-5 shrink-0"
					viewBox="0 0 20 20"
					fill="currentColor"
				>
					<path
						fillRule="evenodd"
						d="M16.704 4.153a.75.75 0 01.143 1.052l-8 10.5a.75.75 0 01-1.127.075l-4.5-4.5a.75.75 0 011.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 011.05-.143z"
						clipRule="evenodd"
					/>
				</svg>
				<span>You're on the list. We'll be in touch.</span>
			</div>
		);
	}

	return (
		<form onSubmit={handleSubmit} className="w-full max-w-md">
			<div className="flex gap-0">
				<div className="relative flex-1">
					<input
						type="email"
						value={email}
						onChange={(e) => setEmail(e.target.value)}
						placeholder="you@company.dev"
						required
						disabled={state === 'loading'}
						className="h-12 w-full rounded-l-lg border border-r-0 border-zinc-700 bg-zinc-900 px-4 font-mono text-sm text-zinc-100 placeholder:text-zinc-600 focus:border-emerald-500/50 focus:outline-none focus:ring-1 focus:ring-emerald-500/20 disabled:opacity-50"
					/>
					{state === 'idle' && (
						<span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 animate-blink font-mono text-emerald-500/60">
							_
						</span>
					)}
				</div>
				<button
					type="submit"
					disabled={state === 'loading'}
					className="group relative h-12 shrink-0 cursor-pointer overflow-hidden rounded-r-lg border border-emerald-500 bg-emerald-500 px-6 font-mono text-sm font-medium text-zinc-950 transition-all hover:bg-emerald-400 hover:border-emerald-400 disabled:cursor-not-allowed disabled:opacity-60"
				>
					<span className="relative z-10">
						{state === 'loading' ? 'Joining...' : 'Join waitlist'}
					</span>
				</button>
			</div>

			{state === 'error' && (
				<div className="animate-fade-in mt-3 flex items-center justify-between rounded-lg border border-red-500/20 bg-red-500/5 px-4 py-2.5 text-sm">
					<span className="font-mono text-red-400">{errorMsg}</span>
					<button
						type="button"
						onClick={() => setState('idle')}
						className="ml-3 cursor-pointer font-mono text-xs text-zinc-500 underline decoration-zinc-700 underline-offset-2 transition-colors hover:text-zinc-300"
					>
						retry
					</button>
				</div>
			)}
		</form>
	);
}
