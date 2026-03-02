import { Form, redirect, useNavigation } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Badge from '~/components/ui/Badge';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import { getSession } from '~/lib/session.server';
import {
	createBillingPortalSession,
	createStripe,
	getOrCreateCustomer,
} from '~/lib/stripe.server';
import type { Route } from './+types/dashboard.billing';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) throw redirect('/auth/login');

	const db = context.cloudflare.env.DB;

	// Get user's team and plan
	const team = await db
		.prepare(
			"SELECT t.id, t.name, t.plan, t.stripe_customer_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role = 'owner' LIMIT 1",
		)
		.bind(session.userId)
		.first<{
			id: string;
			name: string;
			plan: string;
			stripe_customer_id: string | null;
		}>();

	// Get credit balance
	const balance = await db
		.prepare(
			'SELECT COALESCE(SUM(amount), 0) as total FROM credit_ledger WHERE user_id = ?',
		)
		.bind(session.userId)
		.first<{ total: number }>();

	return {
		plan: team?.plan ?? 'free',
		teamName: team?.name ?? '',
		hasPaymentMethod: !!team?.stripe_customer_id,
		creditBalance: balance?.total ?? 0,
	};
}

export async function action({ request, context }: Route.ActionArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) throw redirect('/auth/login');

	const formData = await request.formData();
	const intent = formData.get('intent');
	const env = context.cloudflare.env;
	const db = env.DB;
	const stripe = createStripe(env.STRIPE_SECRET_KEY);

	const user = await db
		.prepare('SELECT email, name FROM users WHERE id = ?')
		.bind(session.userId)
		.first<{ email: string; name: string }>();

	const team = await db
		.prepare(
			"SELECT t.id, t.stripe_customer_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role = 'owner' LIMIT 1",
		)
		.bind(session.userId)
		.first<{ id: string; stripe_customer_id: string | null }>();

	if (!user || !team) throw redirect('/dashboard');

	const customerId = await getOrCreateCustomer(
		stripe,
		db,
		team.id,
		user.email,
		user.name,
	);

	if (intent === 'manage') {
		const url = await createBillingPortalSession(
			stripe,
			customerId,
			env.APP_URL,
		);
		throw redirect(url);
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
			<div className="mt-8 grid grid-cols-1 gap-4 lg:grid-cols-3">
				{plans.map((plan) => {
					const isCurrent = loaderData.plan === plan.current;
					return (
						<Card key={plan.name} accent={isCurrent} className="p-6">
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
							<ul className="mt-6 space-y-2">
								{plan.features.map((f) => (
									<li
										key={f}
										className="flex items-center gap-2 text-sm text-zinc-400"
									>
										<span className="text-emerald-400">&#10003;</span>
										{f}
									</li>
								))}
							</ul>
						</Card>
					);
				})}
			</div>

			{/* Credit balance */}
			<Card className="mt-8 p-6">
				<div className="flex items-center justify-between">
					<div>
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							Credit balance
						</p>
						<p className="mt-2 text-2xl font-bold text-white">
							${loaderData.creditBalance.toFixed(2)}
						</p>
					</div>
					<Button
						variant="secondary"
						onClick={() => {
							window.location.href = '/dashboard/billing/credit';
						}}
					>
						Manage credit
					</Button>
				</div>
			</Card>
		</>
	);
}
