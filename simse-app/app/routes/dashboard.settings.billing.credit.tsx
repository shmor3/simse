import { Link } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard.settings.billing.credit';

export async function loader({ request }: Route.LoaderArgs) {
	try {
		const res = await authenticatedApi(request, '/payments/credits');
		if (!res.ok) return { balance: 0, history: [] };

		type Tx = {
			id: string;
			amount: number;
			description: string;
			created_at: string;
		};
		const json = (await res.json()) as ApiResponse<{
			balance: number;
			history: Tx[];
		}>;
		return {
			balance: json.data?.balance ?? 0,
			history: json.data?.history ?? [],
		};
	} catch {
		return { balance: 0, history: [] };
	}
}

export default function Credit({ loaderData }: Route.ComponentProps) {
	const { balance, history } = loaderData;

	return (
		<>
			<PageHeader
				title="Credit"
				description="View your credit balance and transaction history."
				action={
					<Link to="/dashboard/settings/billing">
						<Button variant="ghost">Back to billing</Button>
					</Link>
				}
			/>

			{/* Balance */}
			<Card
				accent="gradient"
				className="mt-8 p-8 text-center animate-fade-in-up"
			>
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Available credit
				</p>
				<p className="mt-4 font-mono text-5xl font-bold gradient-text">
					${balance.toFixed(2)}
				</p>
			</Card>

			{/* Transaction history */}
			<div className="mt-8">
				<h2 className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Transaction history
				</h2>

				{history.length === 0 ? (
					<Card className="mt-4 p-8 text-center animate-stagger-3">
						<p className="text-sm text-zinc-500">No transactions yet.</p>
						<p className="mt-1 text-[13px] text-zinc-700">
							Your credit history will appear here.
						</p>
					</Card>
				) : (
					<Card className="mt-4 overflow-hidden animate-stagger-3">
						<table className="w-full">
							<thead>
								<tr className="border-b border-zinc-800">
									<th className="px-5 py-3 text-left font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">
										Description
									</th>
									<th className="px-5 py-3 text-right font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">
										Amount
									</th>
									<th className="px-5 py-3 text-right font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">
										Date
									</th>
								</tr>
							</thead>
							<tbody>
								{history.map((tx) => (
									<tr
										key={tx.id}
										className="border-b border-zinc-800/50 transition-colors last:border-0 hover:bg-zinc-800/20"
									>
										<td className="px-5 py-3 text-sm text-zinc-300">
											{tx.description}
										</td>
										<td
											className={`px-5 py-3 text-right font-mono text-sm ${tx.amount >= 0 ? 'text-emerald-400' : 'text-red-400'}`}
										>
											{tx.amount >= 0 ? '+' : ''}$
											{Math.abs(tx.amount).toFixed(2)}
										</td>
										<td className="px-5 py-3 text-right text-sm text-zinc-500">
											{new Date(tx.created_at).toLocaleDateString()}
										</td>
									</tr>
								))}
							</tbody>
						</table>
					</Card>
				)}
			</div>
		</>
	);
}
