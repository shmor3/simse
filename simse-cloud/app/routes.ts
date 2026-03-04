import {
	index,
	layout,
	type RouteConfig,
	route,
} from '@react-router/dev/routes';

export default [
	index('./routes/_index.tsx'),

	layout('./routes/auth.tsx', [
		route('auth/login', './routes/auth.login.tsx'),
		route('auth/register', './routes/auth.register.tsx'),
		route('auth/2fa', './routes/auth.2fa.tsx'),
		route('auth/reset-password', './routes/auth.reset-password.tsx'),
		route('auth/new-password', './routes/auth.new-password.tsx'),
	]),

	route('auth/logout', './routes/auth.logout.tsx'),

	layout('./routes/dashboard.tsx', [
		route('dashboard', './routes/dashboard._index.tsx'),
		route('dashboard/usage', './routes/dashboard.usage.tsx'),
		route('dashboard/billing', './routes/dashboard.billing.tsx'),
		route('dashboard/billing/credit', './routes/dashboard.billing.credit.tsx'),
		route('dashboard/team', './routes/dashboard.team.tsx'),
		route('dashboard/team/plans', './routes/dashboard.team.plans.tsx'),
		route('dashboard/team/invite', './routes/dashboard.team.invite.tsx'),
		route('dashboard/notifications', './routes/dashboard.notifications.tsx'),
		route('dashboard/account', './routes/dashboard.account.tsx'),
	]),
] satisfies RouteConfig;
