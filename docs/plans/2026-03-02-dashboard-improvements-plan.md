# Dashboard Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a header bar with account dropdown, full account settings page, polish all existing dashboard pages, and add responsive mobile sidebar.

**Architecture:** Extend the existing DashboardLayout to include a top header bar with an AccountDropdown component. Add a new `/dashboard/account` route for settings. Add `/auth/logout` action route. Polish all existing pages with better empty states, loading skeletons, hover interactions, and mobile responsiveness.

**Tech Stack:** React Router v7, Tailwind CSS v4, Cloudflare D1, existing component library (Button, Card, Input, Modal, Avatar, Badge)

---

### Task 1: Add logout route

Currently the sidebar posts to `/auth/logout` but no route handles it.

**Files:**
- Create: `simse-cloud/app/routes/auth.logout.tsx`
- Modify: `simse-cloud/app/routes.ts`

**Step 1: Create the logout action route**

```tsx
// simse-cloud/app/routes/auth.logout.tsx
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
```

**Step 2: Register the route**

In `routes.ts`, add after the auth layout block (as a standalone route, not inside the auth layout):

```ts
route('auth/logout', './routes/auth.logout.tsx'),
```

Add it between the auth layout closing `])` and the dashboard layout.

**Step 3: Verify — build passes**

Run: `cd simse-cloud && bun run build`

**Step 4: Commit**

```
feat(simse-cloud): add logout route
```

---

### Task 2: Create AccountDropdown component

**Files:**
- Create: `simse-cloud/app/components/ui/AccountDropdown.tsx`

**Step 1: Create the dropdown component**

```tsx
// simse-cloud/app/components/ui/AccountDropdown.tsx
import { useEffect, useRef, useState } from 'react';
import { Form, Link } from 'react-router';
import Avatar from './Avatar';

interface AccountDropdownProps {
	name: string;
	email: string;
}

export default function AccountDropdown({ name, email }: AccountDropdownProps) {
	const [open, setOpen] = useState(false);
	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (!open) return;
		function onClick(e: MouseEvent) {
			if (ref.current && !ref.current.contains(e.target as Node)) {
				setOpen(false);
			}
		}
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape') setOpen(false);
		}
		document.addEventListener('mousedown', onClick);
		document.addEventListener('keydown', onKey);
		return () => {
			document.removeEventListener('mousedown', onClick);
			document.removeEventListener('keydown', onKey);
		};
	}, [open]);

	return (
		<div ref={ref} className="relative">
			<button
				type="button"
				onClick={() => setOpen((v) => !v)}
				className="flex items-center gap-2.5 rounded-lg px-2 py-1.5 transition-colors hover:bg-zinc-800/60"
			>
				<Avatar name={name} size="sm" />
				<span className="hidden text-sm text-zinc-400 sm:block">{name}</span>
				<svg
					className="h-3.5 w-3.5 text-zinc-600"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					strokeWidth={2}
				>
					<path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
				</svg>
			</button>

			{open && (
				<div className="absolute right-0 top-full z-50 mt-2 w-64 rounded-xl border border-zinc-800 bg-zinc-900 py-1.5 shadow-2xl animate-fade-in">
					{/* User info */}
					<div className="px-4 py-3">
						<p className="text-sm font-medium text-white">{name}</p>
						<p className="mt-0.5 text-[13px] text-zinc-500">{email}</p>
					</div>
					<div className="mx-3 border-t border-zinc-800" />

					{/* Menu items */}
					<div className="py-1.5">
						<Link
							to="/dashboard/account"
							onClick={() => setOpen(false)}
							className="flex items-center gap-3 px-4 py-2 text-sm text-zinc-400 transition-colors hover:bg-zinc-800/50 hover:text-zinc-200"
						>
							<svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
								<path strokeLinecap="round" strokeLinejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
							</svg>
							Account
						</Link>
						<a
							href="https://simse.dev/docs"
							target="_blank"
							rel="noopener noreferrer"
							onClick={() => setOpen(false)}
							className="flex items-center gap-3 px-4 py-2 text-sm text-zinc-400 transition-colors hover:bg-zinc-800/50 hover:text-zinc-200"
						>
							<svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
								<path strokeLinecap="round" strokeLinejoin="round" d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
							</svg>
							Help
							<svg className="ml-auto h-3 w-3 text-zinc-700" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
								<path strokeLinecap="round" strokeLinejoin="round" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
							</svg>
						</a>
					</div>
					<div className="mx-3 border-t border-zinc-800" />

					{/* Sign out */}
					<div className="py-1.5">
						<Form method="post" action="/auth/logout">
							<button
								type="submit"
								className="flex w-full items-center gap-3 px-4 py-2 text-sm text-zinc-400 transition-colors hover:bg-zinc-800/50 hover:text-zinc-200"
							>
								<svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
									<path strokeLinecap="round" strokeLinejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
								</svg>
								Sign out
							</button>
						</Form>
					</div>
				</div>
			)}
		</div>
	);
}
```

**Step 2: Verify — build passes**

Run: `cd simse-cloud && bun run build`

**Step 3: Commit**

```
feat(simse-cloud): add AccountDropdown component
```

---

### Task 3: Add header bar to DashboardLayout + wire user data

**Files:**
- Modify: `simse-cloud/app/components/layout/DashboardLayout.tsx`
- Modify: `simse-cloud/app/components/layout/Sidebar.tsx`
- Modify: `simse-cloud/app/routes/dashboard.tsx`

**Step 1: Update dashboard.tsx loader to return user data**

The loader already has the session. Add a user query:

```tsx
// In dashboard.tsx loader, after the unreadCount query, add:
const user = await context.cloudflare.env.DB.prepare(
	'SELECT name, email FROM users WHERE id = ?',
)
	.bind(session.userId)
	.first<{ name: string; email: string }>();

return {
	unreadCount: result?.count ?? 0,
	userName: user?.name ?? '',
	userEmail: user?.email ?? '',
};
```

Update the component to pass user data:

```tsx
export default function Dashboard({ loaderData }: Route.ComponentProps) {
	return (
		<DashboardLayout
			unreadCount={loaderData.unreadCount}
			userName={loaderData.userName}
			userEmail={loaderData.userEmail}
		/>
	);
}
```

**Step 2: Update DashboardLayout to include header bar**

```tsx
// simse-cloud/app/components/layout/DashboardLayout.tsx
import { Outlet } from 'react-router';
import AccountDropdown from '../ui/AccountDropdown';
import Sidebar from './Sidebar';

interface DashboardLayoutProps {
	unreadCount?: number;
	userName: string;
	userEmail: string;
}

export default function DashboardLayout({
	unreadCount,
	userName,
	userEmail,
}: DashboardLayoutProps) {
	return (
		<div className="flex h-screen bg-[#0a0a0b]">
			<Sidebar unreadCount={unreadCount} userName={userName} />
			<div className="flex flex-1 flex-col overflow-hidden">
				{/* Header bar */}
				<header className="flex h-14 shrink-0 items-center justify-end border-b border-zinc-800/50 px-6">
					<AccountDropdown name={userName} email={userEmail} />
				</header>
				{/* Main content */}
				<main className="flex-1 overflow-y-auto">
					<div className="mx-auto max-w-5xl px-8 py-8">
						<Outlet />
					</div>
				</main>
			</div>
		</div>
	);
}
```

**Step 3: Update Sidebar — add Account nav item, replace bottom sign-out with user display**

In the `nav` array in Sidebar.tsx, add an "Account" item after "Notifications":

```tsx
{
	label: 'Account',
	to: '/dashboard/account',
	icon: (
		<svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
			<path strokeLinecap="round" strokeLinejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
			<path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
		</svg>
	),
},
```

Replace the bottom sign-out section with user avatar display:

```tsx
{/* Bottom — user info */}
<div className="border-t border-zinc-800 p-4">
	<div className="flex items-center gap-3 px-3 py-2">
		<Avatar name={userName} size="sm" />
		<div className="min-w-0 flex-1">
			<p className="truncate text-sm text-zinc-400">{userName}</p>
		</div>
	</div>
</div>
```

Update the SidebarProps to include `userName`:

```tsx
interface SidebarProps {
	unreadCount?: number;
	userName: string;
}
```

Import Avatar at the top of Sidebar.tsx.

**Step 4: Verify — build passes**

Run: `cd simse-cloud && bun run build`

**Step 5: Lint fix**

Run: `cd simse-cloud && bun run lint:fix`

**Step 6: Commit**

```
feat(simse-cloud): add header bar with account dropdown to dashboard
```

---

### Task 4: Create account settings page

**Files:**
- Create: `simse-cloud/app/routes/dashboard.account.tsx`
- Modify: `simse-cloud/app/routes.ts`

**Step 1: Create the account route**

```tsx
// simse-cloud/app/routes/dashboard.account.tsx
import { useState } from 'react';
import { Form, redirect, useNavigation } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import Input from '~/components/ui/Input';
import Modal from '~/components/ui/Modal';
import Avatar from '~/components/ui/Avatar';
import { hashPassword, verifyPassword } from '~/lib/auth.server';
import { getSession } from '~/lib/session.server';
import type { Route } from './+types/dashboard.account';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) throw redirect('/auth/login');

	const db = context.cloudflare.env.DB;
	const user = await db
		.prepare('SELECT id, name, email, created_at FROM users WHERE id = ?')
		.bind(session.userId)
		.first<{ id: string; name: string; email: string; created_at: string }>();

	if (!user) throw redirect('/auth/login');

	return {
		user: {
			name: user.name,
			email: user.email,
			createdAt: user.created_at,
		},
	};
}

export async function action({ request, context }: Route.ActionArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) throw redirect('/auth/login');

	const formData = await request.formData();
	const intent = formData.get('intent');
	const db = context.cloudflare.env.DB;

	if (intent === 'update-name') {
		const name = (formData.get('name') as string)?.trim();
		if (!name || name.length < 2) {
			return { error: 'Name must be at least 2 characters.', intent: 'update-name' };
		}
		await db
			.prepare('UPDATE users SET name = ? WHERE id = ?')
			.bind(name, session.userId)
			.run();
		return { success: true, intent: 'update-name' };
	}

	if (intent === 'change-password') {
		const currentPassword = formData.get('currentPassword') as string;
		const newPassword = formData.get('newPassword') as string;
		const confirmPassword = formData.get('confirmPassword') as string;

		if (!currentPassword || !newPassword || !confirmPassword) {
			return { error: 'All fields are required.', intent: 'change-password' };
		}
		if (newPassword.length < 8) {
			return { error: 'New password must be at least 8 characters.', intent: 'change-password' };
		}
		if (newPassword !== confirmPassword) {
			return { error: 'Passwords do not match.', intent: 'change-password' };
		}

		const user = await db
			.prepare('SELECT password_hash FROM users WHERE id = ?')
			.bind(session.userId)
			.first<{ password_hash: string }>();

		if (!user || !(await verifyPassword(currentPassword, user.password_hash))) {
			return { error: 'Current password is incorrect.', intent: 'change-password' };
		}

		const hash = await hashPassword(newPassword);
		await db
			.prepare('UPDATE users SET password_hash = ? WHERE id = ?')
			.bind(hash, session.userId)
			.run();
		return { success: true, intent: 'change-password' };
	}

	if (intent === 'delete-account') {
		const confirmEmail = (formData.get('confirmEmail') as string)?.trim().toLowerCase();
		const user = await db
			.prepare('SELECT email FROM users WHERE id = ?')
			.bind(session.userId)
			.first<{ email: string }>();

		if (!user || confirmEmail !== user.email.toLowerCase()) {
			return { error: 'Email does not match.', intent: 'delete-account' };
		}

		// Delete user and cascade
		await db.prepare('DELETE FROM sessions WHERE user_id = ?').bind(session.userId).run();
		await db.prepare('DELETE FROM notifications WHERE user_id = ?').bind(session.userId).run();
		await db.prepare('DELETE FROM credit_ledger WHERE user_id = ?').bind(session.userId).run();
		await db.prepare('DELETE FROM team_members WHERE user_id = ?').bind(session.userId).run();
		await db.prepare('DELETE FROM users WHERE id = ?').bind(session.userId).run();

		return redirect('/auth/login');
	}

	return null;
}

export default function Account({ loaderData, actionData }: Route.ComponentProps) {
	const { user } = loaderData;
	const navigation = useNavigation();
	const isSubmitting = navigation.state === 'submitting';
	const [deleteOpen, setDeleteOpen] = useState(false);
	const [confirmEmail, setConfirmEmail] = useState('');

	const ad = actionData as { error?: string; success?: boolean; intent?: string } | undefined;

	return (
		<>
			<PageHeader title="Account" description="Manage your profile, security, and preferences." />

			{/* Profile */}
			<Card className="mt-8 p-6">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Profile
				</p>
				<div className="mt-6 flex items-center gap-4">
					<Avatar name={user.name} size="lg" />
					<div>
						<p className="text-sm font-medium text-white">{user.name}</p>
						<p className="text-[13px] text-zinc-500">{user.email}</p>
					</div>
				</div>

				<Form method="post" className="mt-6 max-w-sm space-y-4">
					<input type="hidden" name="intent" value="update-name" />
					<Input label="Display name" name="name" defaultValue={user.name} error={ad?.intent === 'update-name' ? ad.error : undefined} />
					{ad?.intent === 'update-name' && ad.success && (
						<p className="text-[13px] text-emerald-400">Name updated.</p>
					)}
					<Button type="submit" loading={isSubmitting}>Save</Button>
				</Form>

				<div className="mt-6 border-t border-zinc-800 pt-6">
					<p className="text-[13px] text-zinc-600">
						Member since {new Date(user.createdAt).toLocaleDateString('en', { month: 'long', year: 'numeric' })}
					</p>
				</div>
			</Card>

			{/* Security */}
			<Card className="mt-6 p-6">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Security
				</p>

				<Form method="post" className="mt-6 max-w-sm space-y-4">
					<input type="hidden" name="intent" value="change-password" />
					<Input label="Current password" name="currentPassword" type="password" />
					<Input label="New password" name="newPassword" type="password" />
					<Input label="Confirm new password" name="confirmPassword" type="password" error={ad?.intent === 'change-password' ? ad.error : undefined} />
					{ad?.intent === 'change-password' && ad.success && (
						<p className="text-[13px] text-emerald-400">Password changed.</p>
					)}
					<Button type="submit" variant="secondary" loading={isSubmitting}>Change password</Button>
				</Form>
			</Card>

			{/* Preferences */}
			<Card className="mt-6 p-6">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Preferences
				</p>
				<div className="mt-6 space-y-4">
					{[
						{ id: 'billing', label: 'Billing alerts', desc: 'Payment receipts and failed payment notices' },
						{ id: 'digest', label: 'Weekly digest', desc: 'Summary of your weekly activity' },
						{ id: 'product', label: 'Product updates', desc: 'New features and changelog' },
						{ id: 'security', label: 'Security alerts', desc: 'New device logins and suspicious activity' },
					].map((pref) => (
						<label key={pref.id} className="flex items-center justify-between">
							<div>
								<p className="text-sm text-zinc-200">{pref.label}</p>
								<p className="text-[13px] text-zinc-600">{pref.desc}</p>
							</div>
							<input
								type="checkbox"
								defaultChecked
								className="h-4 w-4 rounded border-zinc-700 bg-zinc-800 text-emerald-400 accent-emerald-400"
							/>
						</label>
					))}
				</div>
			</Card>

			{/* Danger zone */}
			<Card className="mt-6 border-red-500/20 p-6">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-red-400">
					Danger zone
				</p>
				<p className="mt-3 text-sm text-zinc-500">
					Permanently delete your account and all associated data. This action cannot be undone.
				</p>
				<Button variant="danger" className="mt-4" onClick={() => setDeleteOpen(true)}>
					Delete account
				</Button>
			</Card>

			{/* Delete confirmation modal */}
			<Modal
				open={deleteOpen}
				onClose={() => { setDeleteOpen(false); setConfirmEmail(''); }}
				title="Delete account"
				description="This will permanently delete your account, sessions, and data. Type your email to confirm."
				confirmLabel="Delete my account"
				confirmVariant="danger"
				loading={isSubmitting}
			>
				<Form method="post" id="delete-form">
					<input type="hidden" name="intent" value="delete-account" />
					<Input
						name="confirmEmail"
						placeholder={user.email}
						value={confirmEmail}
						onChange={(e) => setConfirmEmail(e.target.value)}
						error={ad?.intent === 'delete-account' ? ad.error : undefined}
					/>
					{/* Hidden submit triggered by modal confirm */}
				</Form>
				{/* Override modal confirm to submit the form */}
				<div className="mt-4 flex justify-end gap-3">
					<Button variant="ghost" onClick={() => { setDeleteOpen(false); setConfirmEmail(''); }}>
						Cancel
					</Button>
					<Button
						variant="danger"
						type="submit"
						form="delete-form"
						loading={isSubmitting}
						disabled={confirmEmail.toLowerCase() !== user.email.toLowerCase()}
					>
						Delete my account
					</Button>
				</div>
			</Modal>
		</>
	);
}
```

Note: The Modal component is used for the overlay/backdrop, but we render our own buttons inside it since we need to submit a form. Pass no `onConfirm` to suppress the default confirm button.

**Step 2: Register the route in routes.ts**

Add inside the dashboard layout block, after the notifications route:

```ts
route('dashboard/account', './routes/dashboard.account.tsx'),
```

**Step 3: Verify — build passes**

Run: `cd simse-cloud && bun run build`

**Step 4: Lint fix**

Run: `cd simse-cloud && bun run lint:fix`

**Step 5: Commit**

```
feat(simse-cloud): add account settings page
```

---

### Task 5: Polish — better empty states

**Files:**
- Modify: `simse-cloud/app/routes/dashboard._index.tsx`
- Modify: `simse-cloud/app/routes/dashboard.notifications.tsx`
- Modify: `simse-cloud/app/routes/dashboard.usage.tsx`

**Step 1: Improve dashboard empty state for sessions**

Replace the current sessions empty state Card (the `recentSessions.length === 0` block) with:

```tsx
<Card className="mt-4 p-10 text-center">
	<div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-zinc-800">
		<svg className="h-6 w-6 text-zinc-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
			<path strokeLinecap="round" strokeLinejoin="round" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
		</svg>
	</div>
	<p className="mt-4 text-sm font-medium text-zinc-400">No sessions yet</p>
	<p className="mt-1 text-[13px] text-zinc-600">Start your first session to see activity here.</p>
</Card>
```

**Step 2: Make quick action cards clickable links**

Wrap each quick action Card in a Link. Replace the 3 quick action cards with:

```tsx
<div className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-3">
	<Link to="#" className="group">
		<Card className="p-5 transition-all group-hover:-translate-y-0.5 group-hover:border-zinc-700">
			<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
				Quick action
			</p>
			<p className="mt-3 text-sm font-semibold text-white">New session</p>
			<p className="mt-1 text-[13px] text-zinc-500">
				Start a fresh AI session with your context.
			</p>
		</Card>
	</Link>
	<Link to="#" className="group">
		<Card className="p-5 transition-all group-hover:-translate-y-0.5 group-hover:border-zinc-700">
			<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
				Quick action
			</p>
			<p className="mt-3 text-sm font-semibold text-white">Browse library</p>
			<p className="mt-1 text-[13px] text-zinc-500">
				Search and manage your knowledge base.
			</p>
		</Card>
	</Link>
	<Link to="/dashboard/team/invite" className="group">
		<Card className="p-5 transition-all group-hover:-translate-y-0.5 group-hover:border-zinc-700">
			<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
				Quick action
			</p>
			<p className="mt-3 text-sm font-semibold text-white">Invite teammate</p>
			<p className="mt-1 text-[13px] text-zinc-500">
				Add someone to your team workspace.
			</p>
		</Card>
	</Link>
</div>
```

Add `Link` to imports from `react-router`.

**Step 3: Improve notifications empty state**

Replace the existing notifications empty Card with:

```tsx
<Card className="mt-8 p-10 text-center">
	<div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-zinc-800">
		<svg className="h-6 w-6 text-zinc-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
			<path strokeLinecap="round" strokeLinejoin="round" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
		</svg>
	</div>
	<p className="mt-4 text-sm font-medium text-zinc-400">All caught up</p>
	<p className="mt-1 text-[13px] text-zinc-600">We'll let you know when something important happens.</p>
</Card>
```

**Step 4: Improve usage breakdown empty state**

Replace the usage breakdown empty state `<p>` with:

```tsx
<div className="flex flex-col items-center py-4">
	<div className="flex h-10 w-10 items-center justify-center rounded-full bg-zinc-800">
		<svg className="h-5 w-5 text-zinc-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
			<path strokeLinecap="round" strokeLinejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
		</svg>
	</div>
	<p className="mt-3 text-sm text-zinc-500">No usage data yet. Start a session to see your breakdown.</p>
</div>
```

**Step 5: Verify — build passes**

Run: `cd simse-cloud && bun run build`

**Step 6: Lint fix**

Run: `cd simse-cloud && bun run lint:fix`

**Step 7: Commit**

```
fix(simse-cloud): improve empty states across dashboard pages
```

---

### Task 6: Polish — bar chart hover tooltips + card interactions

**Files:**
- Modify: `simse-cloud/app/routes/dashboard.usage.tsx`

**Step 1: Add hover tooltips to bar chart bars**

Replace the bar chart `{dailyTokens.map(...)}` block with:

```tsx
{dailyTokens.map((d) => (
	<div
		key={d.day}
		className="group flex flex-1 flex-col items-center gap-2"
	>
		<div
			className="relative w-full flex flex-col items-center justify-end"
			style={{ height: 100 }}
		>
			{/* Tooltip */}
			<div className="pointer-events-none absolute -top-6 left-1/2 -translate-x-1/2 rounded bg-zinc-800 px-2 py-1 opacity-0 transition-opacity group-hover:opacity-100">
				<span className="whitespace-nowrap font-mono text-[10px] text-zinc-300">
					{d.tokens.toLocaleString()}
				</span>
			</div>
			<div
				className="w-full max-w-8 rounded-sm bg-emerald-400/20 transition-all group-hover:bg-emerald-400/40"
				style={{ height: `${Math.max(2, d.pct)}%` }}
			/>
		</div>
		<span className="font-mono text-[10px] text-zinc-600">
			{d.day}
		</span>
	</div>
))}
```

**Step 2: Verify — build passes**

Run: `cd simse-cloud && bun run build`

**Step 3: Commit**

```
fix(simse-cloud): add hover tooltips to usage bar chart
```

---

### Task 7: Mobile responsive sidebar

**Files:**
- Modify: `simse-cloud/app/components/layout/DashboardLayout.tsx`
- Modify: `simse-cloud/app/components/layout/Sidebar.tsx`
- Modify: `simse-cloud/app/styles/app.css`

**Step 1: Add mobile sidebar state to DashboardLayout**

Update DashboardLayout to manage mobile sidebar toggle:

```tsx
import { useState } from 'react';
import { Outlet } from 'react-router';
import AccountDropdown from '../ui/AccountDropdown';
import Sidebar from './Sidebar';

interface DashboardLayoutProps {
	unreadCount?: number;
	userName: string;
	userEmail: string;
}

export default function DashboardLayout({
	unreadCount,
	userName,
	userEmail,
}: DashboardLayoutProps) {
	const [sidebarOpen, setSidebarOpen] = useState(false);

	return (
		<div className="flex h-screen bg-[#0a0a0b]">
			{/* Mobile backdrop */}
			{sidebarOpen && (
				<div
					className="fixed inset-0 z-40 bg-black/60 md:hidden"
					role="presentation"
					onClick={() => setSidebarOpen(false)}
					onKeyDown={(e) => { if (e.key === 'Escape') setSidebarOpen(false); }}
				/>
			)}

			{/* Sidebar */}
			<div className={`fixed inset-y-0 left-0 z-50 w-60 transform transition-transform duration-200 ease-out md:static md:translate-x-0 ${sidebarOpen ? 'translate-x-0' : '-translate-x-full'}`}>
				<Sidebar unreadCount={unreadCount} userName={userName} onClose={() => setSidebarOpen(false)} />
			</div>

			<div className="flex flex-1 flex-col overflow-hidden">
				{/* Header bar */}
				<header className="flex h-14 shrink-0 items-center justify-between border-b border-zinc-800/50 px-6">
					{/* Mobile hamburger */}
					<button
						type="button"
						onClick={() => setSidebarOpen(true)}
						className="rounded-lg p-1.5 text-zinc-500 hover:bg-zinc-800/50 hover:text-zinc-300 md:hidden"
					>
						<svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
							<path strokeLinecap="round" strokeLinejoin="round" d="M4 6h16M4 12h16M4 18h16" />
						</svg>
					</button>
					<div className="hidden md:block" />
					<AccountDropdown name={userName} email={userEmail} />
				</header>
				{/* Main content */}
				<main className="flex-1 overflow-y-auto">
					<div className="mx-auto max-w-5xl px-4 py-6 sm:px-8 sm:py-8">
						<Outlet />
					</div>
				</main>
			</div>
		</div>
	);
}
```

**Step 2: Update Sidebar to accept onClose prop**

Add `onClose` to SidebarProps:

```tsx
interface SidebarProps {
	unreadCount?: number;
	userName: string;
	onClose?: () => void;
}
```

Wrap each NavLink's `className` callback so clicking on mobile also closes the sidebar — add `onClick={() => onClose?.()}` to each NavLink.

**Step 3: Verify — build passes**

Run: `cd simse-cloud && bun run build`

**Step 4: Lint fix**

Run: `cd simse-cloud && bun run lint:fix`

**Step 5: Commit**

```
feat(simse-cloud): add responsive mobile sidebar with hamburger menu
```

---

### Task 8: Final polish + lint + verify

**Files:**
- All modified files

**Step 1: Full build**

Run: `cd simse-cloud && bun run build`

**Step 2: Full lint**

Run: `cd simse-cloud && bun run lint:fix`

**Step 3: Final lint check**

Run: `cd simse-cloud && bun run lint`

**Step 4: Verify route structure**

Check `routes.ts` includes all new routes:
- `auth/logout`
- `dashboard/account`

**Step 5: Commit any remaining fixes**

```
chore(simse-cloud): final lint cleanup
```
