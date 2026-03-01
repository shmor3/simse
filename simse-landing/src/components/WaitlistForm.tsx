import clsx from 'clsx';
import type { FormEvent } from 'react';
import { useState } from 'react';
import { waitlistSchema } from '../lib/schema';

type FormState = 'idle' | 'loading' | 'success' | 'error';

export default function WaitlistForm() {
	const [email, setEmail] = useState('');
	const [state, setState] = useState<FormState>('idle');
	const [errorMsg, setErrorMsg] = useState('');

	async function handleSubmit(e?: FormEvent) {
		e?.preventDefault();

		const result = waitlistSchema.safeParse({ email: email.trim() });
		if (!result.success) {
			setState('error');
			setErrorMsg(result.error.issues[0].message);
			return;
		}

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
			<p
				className={clsx(
					'animate-fade-in text-center font-mono text-sm text-emerald-400',
				)}
			>
				You're on the list.
			</p>
		);
	}

	return (
		<form onSubmit={handleSubmit}>
			<div
				className={clsx(
					'flex flex-col gap-3 sm:flex-row sm:gap-0 sm:overflow-hidden sm:rounded-lg sm:bg-zinc-900',
				)}
			>
				<input
					type="email"
					value={email}
					onChange={(e) => setEmail(e.target.value)}
					placeholder="you@company.dev"
					required
					disabled={state === 'loading'}
					className={clsx(
						'h-14 w-full shrink-0 rounded-lg border border-zinc-700 bg-zinc-800/60 px-4 font-mono text-sm text-zinc-200',
						'placeholder:text-zinc-500 focus:border-zinc-600 focus:outline-none disabled:opacity-50',
						'sm:h-12 sm:min-w-0 sm:shrink sm:flex-1 sm:rounded-none sm:border-none sm:bg-transparent',
					)}
				/>
				<button
					type="submit"
					disabled={state === 'loading'}
					className={clsx(
						'h-14 shrink-0 cursor-pointer rounded-lg bg-emerald-500 px-6 font-mono text-sm font-medium text-zinc-950',
						'transition-colors hover:bg-emerald-400 disabled:cursor-not-allowed disabled:opacity-50',
						'sm:h-12 sm:rounded-none',
					)}
				>
					{state === 'loading' ? 'Joining...' : 'Get early access'}
				</button>
			</div>

			<p
				className={clsx(
					'mt-3 text-center text-[11px] leading-relaxed text-zinc-600',
				)}
			>
				By signing up you agree to receive product updates. Unsubscribe anytime.
			</p>

			{state === 'error' && (
				<p
					className={clsx(
						'animate-fade-in mt-2 text-center font-mono text-xs text-red-400/80',
					)}
				>
					{errorMsg}.{' '}
					<button
						type="button"
						onClick={() => handleSubmit()}
						className={clsx(
							'cursor-pointer underline underline-offset-2 hover:text-red-300',
						)}
					>
						Retry
					</button>
				</p>
			)}
		</form>
	);
}
