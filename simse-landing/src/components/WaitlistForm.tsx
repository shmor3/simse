import type { FormEvent } from 'react';
import { useState } from 'react';

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
				const data = await res
					.json()
					.catch(() => ({ error: 'Request failed' }));
				throw new Error(
					(data as { error?: string }).error || `HTTP ${res.status}`,
				);
			}

			setState('success');
		} catch (err) {
			setState('error');
			setErrorMsg(err instanceof Error ? err.message : 'Something went wrong');
		}
	}

	if (state === 'success') {
		return (
			<p className="animate-fade-in text-center font-mono text-sm text-emerald-400">
				You're on the list.
			</p>
		);
	}

	return (
		<form onSubmit={handleSubmit}>
			<div className="flex flex-col gap-3 sm:flex-row sm:gap-0 sm:overflow-hidden sm:rounded-lg sm:bg-zinc-900">
				<input
					type="email"
					value={email}
					onChange={(e) => setEmail(e.target.value)}
					placeholder="you@company.dev"
					required
					disabled={state === 'loading'}
					className="h-12 min-w-0 flex-1 rounded-lg border-none bg-zinc-900 px-4 font-mono text-sm text-zinc-200 placeholder:text-zinc-600 focus:outline-none disabled:opacity-50 sm:rounded-none sm:bg-transparent"
				/>
				<button
					type="submit"
					disabled={state === 'loading'}
					className="h-12 shrink-0 cursor-pointer rounded-lg bg-emerald-500 px-6 font-mono text-sm font-medium text-zinc-950 transition-colors hover:bg-emerald-400 disabled:cursor-not-allowed disabled:opacity-50 sm:rounded-none"
				>
					{state === 'loading' ? 'Joining...' : 'Get early access'}
				</button>
			</div>

			{state === 'error' && (
				<p className="animate-fade-in mt-2 text-center font-mono text-xs text-red-400/80">
					{errorMsg}.{' '}
					<button
						type="button"
						onClick={() => setState('idle')}
						className="cursor-pointer underline underline-offset-2 hover:text-red-300"
					>
						Retry
					</button>
				</p>
			)}
		</form>
	);
}
