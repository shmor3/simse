import { useState } from 'react';
import { Form } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import Modal from '~/components/ui/Modal';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard.settings.remotes';

type Remote = {
	id: string;
	name: string;
	os: string;
	version: string;
	status: 'connected' | 'offline';
	lastSeen: string;
};

export async function loader({ request }: Route.LoaderArgs) {
	try {
		const res = await authenticatedApi(request, '/remotes');
		if (!res.ok) return { remotes: [] };
		const json = (await res.json()) as ApiResponse<Remote[]>;
		return { remotes: json.data ?? [] };
	} catch {
		return { remotes: [] };
	}
}

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const intent = formData.get('intent');
	const remoteId = formData.get('remoteId') as string;

	if (intent === 'disconnect') {
		await authenticatedApi(request, `/remotes/${remoteId}/disconnect`, {
			method: 'POST',
		});
	}

	if (intent === 'remove') {
		await authenticatedApi(request, `/remotes/${remoteId}`, {
			method: 'DELETE',
		});
	}

	return null;
}

export default function Remotes({ loaderData }: Route.ComponentProps) {
	const { remotes } = loaderData;
	const [connectOpen, setConnectOpen] = useState(false);

	const connected = remotes.filter((r) => r.status === 'connected');
	const offline = remotes.filter((r) => r.status === 'offline');

	return (
		<>
			<PageHeader
				title="Remotes"
				description="simse-remote instances connected to your account"
				action={<Button onClick={() => setConnectOpen(true)}>+ Connect</Button>}
			/>

			{connected.length > 0 && (
				<Card className="mt-8 overflow-hidden">
					<div className="border-b border-zinc-800 px-6 py-4">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							Connected ({connected.length})
						</p>
					</div>
					<div className="divide-y divide-zinc-800/50">
						{connected.map((remote) => (
							<div
								key={remote.id}
								className="flex items-center justify-between px-6 py-4"
							>
								<div className="flex items-center gap-3">
									<span className="h-2.5 w-2.5 rounded-full bg-emerald-400" />
									<div>
										<p className="text-sm font-medium text-white">
											{remote.name}
										</p>
										<p className="text-[12px] text-zinc-600">
											{remote.os} &middot; v{remote.version} &middot; Connected{' '}
											{new Date(remote.lastSeen).toLocaleDateString()}
										</p>
									</div>
								</div>
								<Form method="post">
									<input type="hidden" name="intent" value="disconnect" />
									<input type="hidden" name="remoteId" value={remote.id} />
									<Button variant="ghost" type="submit" className="text-[12px]">
										Disconnect
									</Button>
								</Form>
							</div>
						))}
					</div>
				</Card>
			)}

			{offline.length > 0 && (
				<Card className="mt-6 overflow-hidden">
					<div className="border-b border-zinc-800 px-6 py-4">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							Offline ({offline.length})
						</p>
					</div>
					<div className="divide-y divide-zinc-800/50">
						{offline.map((remote) => (
							<div
								key={remote.id}
								className="flex items-center justify-between px-6 py-4"
							>
								<div className="flex items-center gap-3">
									<span className="h-2.5 w-2.5 rounded-full bg-zinc-600" />
									<div>
										<p className="text-sm text-zinc-300">{remote.name}</p>
										<p className="text-[12px] text-zinc-600">
											{remote.os} &middot; v{remote.version} &middot; Last seen{' '}
											{new Date(remote.lastSeen).toLocaleDateString()}
										</p>
									</div>
								</div>
								<Form method="post">
									<input type="hidden" name="intent" value="remove" />
									<input type="hidden" name="remoteId" value={remote.id} />
									<Button
										variant="danger"
										type="submit"
										className="text-[12px]"
									>
										Remove
									</Button>
								</Form>
							</div>
						))}
					</div>
				</Card>
			)}

			{remotes.length === 0 && (
				<Card className="mt-8 p-8 text-center">
					<p className="text-sm text-zinc-500">
						No remotes connected yet. Click &ldquo;+ Connect&rdquo; to get
						started.
					</p>
				</Card>
			)}

			<Modal
				open={connectOpen}
				onClose={() => setConnectOpen(false)}
				title="Connect a remote"
				description="Install simse-remote on your machine and authenticate."
			>
				<div className="space-y-3">
					<div className="rounded-lg bg-zinc-800 p-3">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-500">
							1. Install
						</p>
						<pre className="mt-1.5 text-[13px] text-zinc-300">
							curl -fsSL https://simse.dev/install | sh
						</pre>
					</div>
					<div className="rounded-lg bg-zinc-800 p-3">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-500">
							2. Authenticate
						</p>
						<pre className="mt-1.5 text-[13px] text-zinc-300">
							simse auth login
						</pre>
					</div>
					<div className="rounded-lg bg-zinc-800 p-3">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-500">
							3. Connect
						</p>
						<pre className="mt-1.5 text-[13px] text-zinc-300">
							simse remote connect
						</pre>
					</div>
				</div>
			</Modal>
		</>
	);
}
