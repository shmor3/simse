import { redirect } from 'react-router';
import { authenticatedApi } from '~/lib/api.server';
import { clearSessionCookie } from '~/lib/session.server';
import type { Route } from './+types/auth.logout';

export async function action({ request }: Route.ActionArgs) {
	try {
		await authenticatedApi(request, '/auth/logout', { method: 'POST' });
	} catch {
		// If auth fails, still clear the cookie
	}
	return redirect('/auth/login', {
		headers: { 'Set-Cookie': clearSessionCookie() },
	});
}

export function loader() {
	return redirect('/');
}
