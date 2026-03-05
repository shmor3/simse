import clsx from 'clsx';
import type { Route } from './+types/unsubscribe';

export async function loader({ request, context }: Route.LoaderArgs) {
	const url = new URL(request.url);
	const email = url.searchParams.get('email')?.trim().toLowerCase();

	if (!email) {
		return { success: false, message: 'Invalid unsubscribe link' };
	}

	try {
		await context.cloudflare.env.DB.prepare(
			"UPDATE waitlist SET subscribed = 0, updated_at = datetime('now') WHERE email = ? AND subscribed = 1",
		)
			.bind(email)
			.run();
	} catch (err) {
		console.error('D1 update failed', err);
		return { success: false, message: 'Something went wrong' };
	}

	return { success: true, message: 'Unsubscribed' };
}

export default function Unsubscribe({ loaderData }: Route.ComponentProps) {
	const { success, message } = loaderData;

	return (
		<div className="flex min-h-screen items-center justify-center bg-[#0a0a0b] px-8">
			<div className="max-w-[420px] text-center">
				<div
					className={clsx(
						'text-[2rem]',
						success ? 'text-emerald-400' : 'text-red-400',
					)}
				>
					{success ? '\u2713' : '\u2717'}
				</div>
				<h1
					className={clsx(
						'mt-4 text-xl font-semibold',
						success ? 'text-emerald-400' : 'text-red-400',
					)}
				>
					{message}
				</h1>
				<p className="mt-3 text-sm leading-relaxed text-zinc-500">
					{success
						? "You've been removed from our mailing list and won't receive any more emails from us."
						: 'Please try again or contact us if the issue persists.'}
				</p>
				<p className="mt-6">
					<a
						href="/"
						className="text-zinc-400 underline underline-offset-2 hover:text-zinc-300"
					>
						Back to simse.dev
					</a>
				</p>
			</div>
		</div>
	);
}
