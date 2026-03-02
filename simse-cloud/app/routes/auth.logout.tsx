import { redirect } from 'react-router';
import { deleteSession } from '~/lib/auth.server';
import { clearSessionCookie, getSession } from '~/lib/session.server';
import type { Route } from './+types/auth.logout';

export async function action({ request, context }: Route.ActionArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (session) {
		await deleteSession(context.cloudflare.env.DB, session.sessionId);
	}
	return redirect('/auth/login', {
		headers: { 'Set-Cookie': clearSessionCookie() },
	});
}

export function loader() {
	return redirect('/');
}
