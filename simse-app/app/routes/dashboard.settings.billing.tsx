import clsx from 'clsx';
import { Form, redirect, useNavigation } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Badge from '~/components/ui/Badge';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard.settings.billing';

export async function loader({ request }: Route.LoaderArgs) {
	const res = await authenticatedApi(request, '/payments/billing');
	if (!res.ok) throw redirect('/auth/login');

	const json = (await res.json()) as ApiResponse<{
		plan: string;
		teamName: string;
		hasPaymentMethod: boolean;
		creditBalance: number;
	}>;
	const data = json.data;

	return {
		plan: data?.plan ?? 'free',
		teamName: data?.teamName ?? '',
		hasPaymentMethod: !!data?.hasPaymentMethod,
		creditBalance: data?.creditBalance ?? 0,
	};
}

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const intent = formData.get('intent');

	if (intent === 'manage') {
		const res = await authenticatedApi(request, '/payments/portal', {
			method: 'POST',
		});

		if (res.ok) {
			const json = (await res.json()) as ApiResponse<{ url?: string }>;
			if (json.data?.url) throw redirect(json.data.url);
		}
	}

	return null;
}

const plans = [
	{
		name: 'Free',
		price: '$0',
		period: 'forever',
		features: ['1,000 tokens/month', '1 team member', 'Basic support'],
		current: 'free',
	},
	{
		name: 'Pro',
		price: '$24',
		period: '/month',
		features: [
			'100,000 tokens/month',
			'5 team members',
			'Priority support',
			'Library access',
		],
		current: 'pro',
	},
	{
		name: 'Team',
		price: '$99',
		period: '/month',
		features: [
			'500,000 tokens/month',
			'Unlimited members',
			'Custom integrations',
			'Dedicated support',
		],
		current: 'team',
	},
];

export default function Billing({ loaderData }: Route.ComponentProps) {
	const navigation = useNavigation();
	const isSubmitting = navigation.state === 'submitting';

	return (
		<>
			<PageHeader
				title="Billing"
				description="Manage your subscription and payment methods."
				action={
					loaderData.hasPaymentMethod ? (
						<Form method="post">
							<input type="hidden" name="intent" value="manage" />
							<Button variant="secondary" type="submit" loading={isSubmitting}>
								Manage billing
							</Button>
						</Form>
					) : undefined
				}
			/>

			{/* Plan cards */}
			<div className="mt-8 grid grid-cols-1 gap-4 lg:grid-cols-3 animate-fade-in-up">
				{plans.map((plan) => {
					const isCurrent = loaderData.plan === plan.current;
					return (
						<Card
							key={plan.name}
							accent={isCurrent ? 'gradient' : undefined}
							className={clsx(
								'card-hover p-6',
								isCurrent && 'ring-1 ring-emerald-400/20',
							)}
						>
							<div className="flex items-center justify-between">
								<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
									{plan.name}
								</p>
								{isCurrent && <Badge variant="emerald">Current</Badge>}
							</div>
							<p className="mt-4">
								<span className="text-3xl font-bold text-white">
									{plan.price}
								</span>
								<span className="text-sm text-zinc-500">{plan.period}</span>
							</p>
							<ul className="mt-6 space-y-2.5">
								{plan.features.map((f) => (
									<li
										key={f}
										className="flex items-center gap-2.5 text-sm text-zinc-400"
									>
										<svg
											className="h-3.5 w-3.5 shrink-0 text-emerald-400"
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
										{f}
									</li>
								))}
							</ul>
							{!isCurrent && (
								<Button
									variant="secondary"
									className="mt-6 w-full"
									onClick={() => {}}
								>
									Upgrade
								</Button>
							)}
						</Card>
					);
				})}
			</div>

			{/* Credit balance */}
			<Card className="mt-8 card-hover p-6 animate-stagger-3">
				<div className="flex items-center justify-between">
					<div>
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							Credit balance
						</p>
						<p className="mt-2 text-2xl font-bold text-emerald-400">
							${loaderData.creditBalance.toFixed(2)}
						</p>
					</div>
					<Button
						variant="secondary"
						onClick={() => {
							window.location.href = '/dashboard/settings/billing/credit';
						}}
					>
						Manage credit
					</Button>
				</div>
			</Card>
		</>
	);
}
