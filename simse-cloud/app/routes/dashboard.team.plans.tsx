import { Link } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Badge from '~/components/ui/Badge';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard.team.plans';

export async function loader({ request }: Route.LoaderArgs) {
	try {
		const res = await authenticatedApi(request, '/payments/billing');
		if (!res.ok) return { currentPlan: 'free' };

		const json = (await res.json()) as ApiResponse<{ plan: string }>;
		return { currentPlan: json.data?.plan ?? 'free' };
	} catch {
		return { currentPlan: 'free' };
	}
}

const plans = [
	{
		id: 'free',
		name: 'Free',
		price: '$0',
		period: '/month',
		description: 'For individuals getting started',
		features: [
			{ name: 'Tokens', value: '1,000/mo' },
			{ name: 'Team members', value: '1' },
			{ name: 'Library items', value: '100' },
			{ name: 'Support', value: 'Community' },
			{ name: 'Sessions', value: '10/mo' },
		],
	},
	{
		id: 'pro',
		name: 'Pro',
		price: '$24',
		period: '/month',
		description: 'For professionals and small teams',
		features: [
			{ name: 'Tokens', value: '100,000/mo' },
			{ name: 'Team members', value: '5' },
			{ name: 'Library items', value: '10,000' },
			{ name: 'Support', value: 'Priority' },
			{ name: 'Sessions', value: 'Unlimited' },
		],
	},
	{
		id: 'team',
		name: 'Team',
		price: '$99',
		period: '/month',
		description: 'For growing organizations',
		features: [
			{ name: 'Tokens', value: '500,000/mo' },
			{ name: 'Team members', value: 'Unlimited' },
			{ name: 'Library items', value: '100,000' },
			{ name: 'Support', value: 'Dedicated' },
			{ name: 'Sessions', value: 'Unlimited' },
		],
	},
];

export default function TeamPlans({ loaderData }: Route.ComponentProps) {
	return (
		<>
			<PageHeader
				title="Plans"
				description="Compare plans and choose the right one for your team."
				action={
					<Link to="/dashboard/team">
						<Button variant="ghost">Back</Button>
					</Link>
				}
			/>

			<div className="mt-8 grid grid-cols-1 gap-4 lg:grid-cols-3">
				{plans.map((plan) => {
					const isCurrent = loaderData.currentPlan === plan.id;
					return (
						<Card
							key={plan.id}
							accent={isCurrent}
							className="flex flex-col p-6"
						>
							<div className="flex items-center justify-between">
								<p className="text-lg font-bold text-white">{plan.name}</p>
								{isCurrent && <Badge variant="emerald">Current</Badge>}
							</div>
							<p className="mt-1 text-sm text-zinc-500">{plan.description}</p>
							<p className="mt-4">
								<span className="text-3xl font-bold text-white">
									{plan.price}
								</span>
								<span className="text-sm text-zinc-500">{plan.period}</span>
							</p>

							<div className="mt-6 flex-1 space-y-3">
								{plan.features.map((f) => (
									<div
										key={f.name}
										className="flex items-center justify-between text-sm"
									>
										<span className="text-zinc-500">{f.name}</span>
										<span className="font-medium text-zinc-300">{f.value}</span>
									</div>
								))}
							</div>

							<div className="mt-6">
								{isCurrent ? (
									<Button variant="secondary" className="w-full" disabled>
										Current plan
									</Button>
								) : (
									<Link to="/dashboard/billing">
										<Button
											variant={plan.id === 'pro' ? 'primary' : 'secondary'}
											className="w-full"
										>
											{plan.id === 'free' ? 'Downgrade' : 'Upgrade'}
										</Button>
									</Link>
								)}
							</div>
						</Card>
					);
				})}
			</div>
		</>
	);
}
