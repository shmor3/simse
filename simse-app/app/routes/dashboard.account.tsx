import { useState } from 'react';
import { Form, redirect, useNavigation, useSubmit } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Avatar from '~/components/ui/Avatar';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import Input from '~/components/ui/Input';
import Modal from '~/components/ui/Modal';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';
import { clearSessionCookie } from '~/lib/session.server';
import type { Route } from './+types/dashboard.account';

export async function loader({ request }: Route.LoaderArgs) {
	const res = await authenticatedApi(request, '/auth/me');
	if (!res.ok) throw redirect('/auth/login');

	const json = (await res.json()) as ApiResponse<{
		name: string;
		email: string;
		createdAt: string;
	}>;
	const user = json.data;

	return {
		user: {
			name: user?.name ?? '',
			email: user?.email ?? '',
			createdAt: user?.createdAt ?? '',
		},
	};
}

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const intent = formData.get('intent');

	if (intent === 'update-name') {
		const name = (formData.get('name') as string)?.trim();
		if (!name || name.length < 2) {
			return {
				error: 'Name must be at least 2 characters.',
				intent: 'update-name',
			};
		}
		await authenticatedApi(request, '/users/me/name', {
			method: 'PUT',
			body: JSON.stringify({ name }),
		});
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
			return {
				error: 'New password must be at least 8 characters.',
				intent: 'change-password',
			};
		}
		if (newPassword !== confirmPassword) {
			return { error: 'Passwords do not match.', intent: 'change-password' };
		}

		const res = await authenticatedApi(request, '/users/me/password', {
			method: 'PUT',
			body: JSON.stringify({ currentPassword, newPassword }),
		});

		if (!res.ok) {
			return {
				error: 'Current password is incorrect.',
				intent: 'change-password',
			};
		}

		return { success: true, intent: 'change-password' };
	}

	if (intent === 'delete-account') {
		const confirmEmail = (formData.get('confirmEmail') as string)
			?.trim()
			.toLowerCase();

		const res = await authenticatedApi(request, '/users/me', {
			method: 'DELETE',
			body: JSON.stringify({ confirmEmail }),
		});

		if (!res.ok) {
			return { error: 'Email does not match.', intent: 'delete-account' };
		}

		return redirect('/auth/login', {
			headers: { 'Set-Cookie': clearSessionCookie() },
		});
	}

	return null;
}

export default function Account({
	loaderData,
	actionData,
}: Route.ComponentProps) {
	const { user } = loaderData;
	const navigation = useNavigation();
	const submit = useSubmit();
	const isSubmitting = navigation.state === 'submitting';
	const [deleteOpen, setDeleteOpen] = useState(false);
	const [confirmEmail, setConfirmEmail] = useState('');

	const ad = actionData as
		| { error?: string; success?: boolean; intent?: string }
		| undefined;

	return (
		<>
			<PageHeader
				title="Account"
				description="Manage your profile, security, and preferences."
			/>

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
					<Input
						label="Display name"
						name="name"
						defaultValue={user.name}
						error={ad?.intent === 'update-name' ? ad.error : undefined}
					/>
					{ad?.intent === 'update-name' && ad.success && (
						<p className="text-[13px] text-emerald-400">Name updated.</p>
					)}
					<Button type="submit" loading={isSubmitting}>
						Save
					</Button>
				</Form>

				<div className="mt-6 border-t border-zinc-800 pt-6">
					<p className="text-[13px] text-zinc-600">
						Member since{' '}
						{new Date(user.createdAt).toLocaleDateString('en', {
							month: 'long',
							year: 'numeric',
						})}
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
					<Input
						label="Current password"
						name="currentPassword"
						type="password"
					/>
					<Input label="New password" name="newPassword" type="password" />
					<Input
						label="Confirm new password"
						name="confirmPassword"
						type="password"
						error={ad?.intent === 'change-password' ? ad.error : undefined}
					/>
					{ad?.intent === 'change-password' && ad.success && (
						<p className="text-[13px] text-emerald-400">Password changed.</p>
					)}
					<Button type="submit" variant="secondary" loading={isSubmitting}>
						Change password
					</Button>
				</Form>
			</Card>

			{/* Danger zone */}
			<Card className="mt-6 border-red-500/20 p-6">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-red-400">
					Danger zone
				</p>
				<p className="mt-3 text-sm text-zinc-500">
					Permanently delete your account and all associated data. This action
					cannot be undone.
				</p>
				<Button
					variant="danger"
					className="mt-4"
					onClick={() => setDeleteOpen(true)}
				>
					Delete account
				</Button>
			</Card>

			{/* Delete confirmation modal */}
			<Modal
				open={deleteOpen}
				onClose={() => {
					setDeleteOpen(false);
					setConfirmEmail('');
				}}
				title="Delete account"
				description="This will permanently delete your account, sessions, and data. Type your email to confirm."
				confirmLabel="Delete my account"
				confirmVariant="danger"
				loading={isSubmitting}
				onConfirm={() => {
					const formData = new FormData();
					formData.set('intent', 'delete-account');
					formData.set('confirmEmail', confirmEmail);
					submit(formData, { method: 'post' });
				}}
			>
				<Input
					placeholder={user.email}
					value={confirmEmail}
					onChange={(e) => setConfirmEmail(e.target.value)}
					error={ad?.intent === 'delete-account' ? ad.error : undefined}
				/>
			</Modal>
		</>
	);
}
