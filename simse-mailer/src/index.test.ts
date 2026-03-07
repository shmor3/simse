import { SELF } from 'cloudflare:test';
import { describe, expect, it } from 'vitest';

describe('GET /health', () => {
	it('returns 200 with ok: true', async () => {
		const res = await SELF.fetch('https://mailer.test/health');
		expect(res.status).toBe(200);
		const body = await res.json<{ ok: boolean }>();
		expect(body.ok).toBe(true);
	});
});

describe('Template rendering', () => {
	it('renders all registered templates without error', async () => {
		const { renderEmail, templateNames } = await import('./emails/index');

		const testProps: Record<string, Record<string, unknown>> = {
			verify: {
				verificationCode: '123456',
				verificationUrl: 'https://test.dev/verify',
			},
			'two-factor': { code: '789012' },
			'reset-password': { resetUrl: 'https://test.dev/reset' },
			'email-change': { confirmUrl: 'https://test.dev/confirm' },
			'new-device': {
				device: 'Chrome on macOS',
				location: 'San Francisco',
				time: '2026-03-06 10:00',
			},
			'suspicious-activity': {
				activity: 'Login attempt',
				location: 'Unknown',
				time: '2026-03-06 10:00',
			},
			onboarding: {},
			'payment-receipt': {
				amount: '$29',
				planName: 'Pro',
				date: '2026-03-06',
				invoiceUrl: 'https://test.dev/invoice',
			},
			'payment-failed': {
				amount: '$29',
				retryDate: '2026-03-13',
				updateUrl: 'https://test.dev/billing',
			},
			'usage-warning': {
				usedPercent: '80',
				upgradeUrl: 'https://test.dev/upgrade',
			},
			'free-credit': {
				creditAmount: '$10',
				expiryDate: '2026-06-06',
			},
			'weekly-digest': {
				sessions: '12',
				tokensUsed: '45,000',
				libraryItems: '3',
			},
			'feature-announcement': {
				title: 'New Feature',
				features: [
					{ title: 'Feature A', description: 'Does A' },
					{ title: 'Feature B', description: 'Does B' },
				],
				changelogUrl: 'https://test.dev/changelog',
			},
			're-engagement': { daysSinceLogin: '14' },
			'team-invite': {
				inviterName: 'Alice',
				teamName: 'Acme',
				inviteUrl: 'https://test.dev/invite',
			},
			'role-change': {
				teamName: 'Acme',
				newRole: 'admin',
				changedBy: 'Bob',
			},
			'invite-friend': {
				referrerName: 'Alice',
				inviteUrl: 'https://test.dev/refer',
			},
			'waitlist-welcome': {},
			'waitlist-early-preview': {},
			'waitlist-invite': {},
		};

		expect(templateNames.length).toBe(20);

		for (const name of templateNames) {
			const props = testProps[name] ?? {};
			const result = await renderEmail(name, props);
			expect(result.subject).toBeTruthy();
			expect(result.html).toContain('<!DOCTYPE html');
		}
	});

	it('generates correct subject lines', async () => {
		const { renderEmail } = await import('./emails/index');

		const result = await renderEmail('verify', {
			verificationCode: '999888',
		});
		expect(result.subject).toBe('Your simse verification code: 999888');

		const result2 = await renderEmail('team-invite', {
			inviterName: 'Bob',
			teamName: 'DevTeam',
		});
		expect(result2.subject).toBe('Bob invited you to join DevTeam on simse');
	});

	it('throws for unknown template', async () => {
		const { renderEmail } = await import('./emails/index');
		await expect(renderEmail('nonexistent')).rejects.toThrow(
			'Unknown email template: nonexistent',
		);
	});

	it('includes waitlist templates in registry', async () => {
		const { templateNames } = await import('./emails/index');
		expect(templateNames).toContain('waitlist-welcome');
		expect(templateNames).toContain('waitlist-early-preview');
		expect(templateNames).toContain('waitlist-invite');
	});
});
