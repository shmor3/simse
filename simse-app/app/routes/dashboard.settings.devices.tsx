import { Form } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Badge from '~/components/ui/Badge';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard.settings.devices';

type Device = {
	id: string;
	browser: string;
	os: string;
	location: string;
	lastActive: string;
	current: boolean;
};

export async function loader({ request }: Route.LoaderArgs) {
	try {
		const res = await authenticatedApi(request, '/auth/devices');
		if (!res.ok) return { devices: [] };
		const json = (await res.json()) as ApiResponse<Device[]>;
		return { devices: json.data ?? [] };
	} catch {
		return { devices: [] };
	}
}

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const intent = formData.get('intent');

	if (intent === 'revoke') {
		const deviceId = formData.get('deviceId') as string;
		await authenticatedApi(request, `/auth/devices/${deviceId}`, {
			method: 'DELETE',
		});
	}

	if (intent === 'revoke-all') {
		await authenticatedApi(request, '/auth/devices', {
			method: 'DELETE',
		});
	}

	return null;
}

export default function Devices({ loaderData }: Route.ComponentProps) {
	const { devices } = loaderData;
	const current = devices.find((d) => d.current);
	const others = devices.filter((d) => !d.current);

	return (
		<>
			<PageHeader
				title="Devices"
				description="Browsers and apps signed into your account"
			/>

			{current && (
				<Card accent className="mt-8">
					<div className="px-6 py-4">
						<div className="flex items-center justify-between">
							<div className="flex items-center gap-3">
								<div className="flex h-9 w-9 items-center justify-center rounded-lg bg-emerald-400/10 text-emerald-400">
									<svg
										className="h-4 w-4"
										fill="none"
										viewBox="0 0 24 24"
										stroke="currentColor"
										strokeWidth={2}
									>
										<path
											strokeLinecap="round"
											strokeLinejoin="round"
											d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
										/>
									</svg>
								</div>
								<div>
									<p className="text-sm font-medium text-white">
										{current.browser} on {current.os}
									</p>
									<p className="text-[13px] text-zinc-500">
										{current.location}
									</p>
								</div>
							</div>
							<Badge variant="emerald">Current session</Badge>
						</div>
					</div>
				</Card>
			)}

			{others.length > 0 && (
				<Card className="mt-6 overflow-hidden">
					<div className="border-b border-zinc-800 px-6 py-4">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							Other devices ({others.length})
						</p>
					</div>
					<div className="divide-y divide-zinc-800/50">
						{others.map((device) => (
							<div
								key={device.id}
								className="flex items-center justify-between px-6 py-4"
							>
								<div className="flex items-center gap-3">
									<div className="flex h-9 w-9 items-center justify-center rounded-lg bg-zinc-800 text-zinc-400">
										<svg
											className="h-4 w-4"
											fill="none"
											viewBox="0 0 24 24"
											stroke="currentColor"
											strokeWidth={2}
										>
											<path
												strokeLinecap="round"
												strokeLinejoin="round"
												d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
											/>
										</svg>
									</div>
									<div>
										<p className="text-sm text-zinc-200">
											{device.browser} on {device.os}
										</p>
										<p className="text-[12px] text-zinc-600">
											{device.location} &middot; Last active{' '}
											{new Date(device.lastActive).toLocaleDateString()}
										</p>
									</div>
								</div>
								<Form method="post">
									<input type="hidden" name="intent" value="revoke" />
									<input type="hidden" name="deviceId" value={device.id} />
									<Button
										variant="danger"
										type="submit"
										className="text-[12px]"
									>
										Revoke
									</Button>
								</Form>
							</div>
						))}
					</div>
				</Card>
			)}

			{others.length > 0 && (
				<Form method="post" className="mt-6">
					<input type="hidden" name="intent" value="revoke-all" />
					<Button variant="danger" type="submit">
						Sign out all other devices
					</Button>
				</Form>
			)}

			{devices.length === 0 && (
				<Card className="mt-8 p-6">
					<p className="text-center text-sm text-zinc-500">
						No device information available.
					</p>
				</Card>
			)}
		</>
	);
}
