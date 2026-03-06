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
		route('dashboard/notifications', './routes/dashboard.notifications.tsx'),
		route('dashboard/account', './routes/dashboard.account.tsx'),
		layout('./routes/dashboard.settings.tsx', [
			route('dashboard/settings', './routes/dashboard.settings._index.tsx'),
			route(
				'dashboard/settings/billing',
				'./routes/dashboard.settings.billing.tsx',
			),
			route(
				'dashboard/settings/billing/credit',
				'./routes/dashboard.settings.billing.credit.tsx',
			),
			route('dashboard/settings/team', './routes/dashboard.settings.team.tsx'),
			route(
				'dashboard/settings/team/plans',
				'./routes/dashboard.settings.team.plans.tsx',
			),
			route(
				'dashboard/settings/team/invite',
				'./routes/dashboard.settings.team.invite.tsx',
			),
		]),
	]),
] satisfies RouteConfig;
