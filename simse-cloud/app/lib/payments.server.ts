interface PaymentsClientOptions {
	apiUrl: string;
	apiSecret: string;
}

async function request<T>(
	opts: PaymentsClientOptions,
	method: string,
	path: string,
	body?: unknown,
): Promise<T> {
	const res = await fetch(`${opts.apiUrl}${path}`, {
		method,
		headers: {
			Authorization: `Bearer ${opts.apiSecret}`,
			'Content-Type': 'application/json',
		},
		body: body ? JSON.stringify(body) : undefined,
	});

	if (!res.ok) {
		const text = await res.text();
		throw new Error(`Payments API error (${res.status}): ${text}`);
	}

	return res.json() as Promise<T>;
}

export function createPaymentsClient(opts: PaymentsClientOptions) {
	return {
		getOrCreateCustomer(teamId: string, email: string, name: string) {
			return request<{ customerId: string }>(opts, 'POST', '/customers', {
				teamId,
				email,
				name,
			});
		},

		createCheckoutSession(teamId: string, priceId: string, appUrl: string) {
			return request<{ url: string }>(opts, 'POST', '/checkout', {
				teamId,
				priceId,
				appUrl,
			});
		},

		createPortalSession(teamId: string, appUrl: string) {
			return request<{ url: string }>(opts, 'POST', '/portal', {
				teamId,
				appUrl,
			});
		},

		getSubscription(teamId: string) {
			return request<{
				teamId: string;
				plan: string;
				status: string;
				stripeSubscriptionId: string | null;
			}>(opts, 'GET', `/subscriptions/${teamId}`);
		},

		getCredits(userId: string) {
			return request<{
				balance: number;
				history: Array<{
					id: string;
					amount: number;
					description: string;
					created_at: string;
				}>;
			}>(opts, 'GET', `/credits/${userId}`);
		},

		getUsage(userId: string) {
			return request<{
				balance: number;
				recentUsage: Array<{ day: string; tokens: number }>;
			}>(opts, 'GET', `/credits/${userId}/usage`);
		},

		addCredit(userId: string, amount: number, description: string) {
			return request<{ id: string; balance: number }>(
				opts,
				'POST',
				'/credits',
				{ userId, amount, description },
			);
		},
	};
}
