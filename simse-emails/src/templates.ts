import type { ComponentType } from 'react';
import EmailChange from './templates/email-change';
import FeatureAnnouncement from './templates/feature-announcement';
import FreeCredit from './templates/free-credit';
import InviteFriend from './templates/invite-friend';
import NewDevice from './templates/new-device';
import Onboarding from './templates/onboarding';
import PaymentFailed from './templates/payment-failed';
import PaymentReceipt from './templates/payment-receipt';
import ReEngagement from './templates/re-engagement';
import ResetPassword from './templates/reset-password';
import RoleChange from './templates/role-change';
import SuspiciousActivity from './templates/suspicious-activity';
import TeamInvite from './templates/team-invite';
import TwoFactor from './templates/two-factor';
import UsageWarning from './templates/usage-warning';
import Verify from './templates/verify';
import WeeklyDigest from './templates/weekly-digest';

interface TemplateEntry {
	// biome-ignore lint/suspicious/noExplicitAny: template props vary per component
	component: ComponentType<any>;
	// biome-ignore lint/suspicious/noExplicitAny: props shape varies per template
	subject: (props: any) => string;
}

export const templates: Record<string, TemplateEntry> = {
	verify: {
		component: Verify,
		subject: (p) => `Your simse verification code: ${p.verificationCode}`,
	},
	'two-factor': {
		component: TwoFactor,
		subject: (p) => `Your simse login code: ${p.code}`,
	},
	'reset-password': {
		component: ResetPassword,
		subject: () => 'Reset your simse password',
	},
	'email-change': {
		component: EmailChange,
		subject: () => 'Confirm your new simse email address',
	},
	'new-device': {
		component: NewDevice,
		subject: (p) => `New sign-in to your simse account from ${p.device}`,
	},
	'suspicious-activity': {
		component: SuspiciousActivity,
		subject: () => 'Unusual activity detected on your simse account',
	},
	onboarding: {
		component: Onboarding,
		subject: () => 'Three things to try in your first simse session',
	},
	'payment-receipt': {
		component: PaymentReceipt,
		subject: (p) => `Receipt for your simse ${p.planName} plan — ${p.amount}`,
	},
	'payment-failed': {
		component: PaymentFailed,
		subject: (p) => `Your simse payment of ${p.amount} didn't go through`,
	},
	'usage-warning': {
		component: UsageWarning,
		subject: (p) =>
			`You've used ${p.usedPercent}% of your simse credit this cycle`,
	},
	'free-credit': {
		component: FreeCredit,
		subject: (p) => `You've got ${p.creditAmount} in free simse credit`,
	},
	'weekly-digest': {
		component: WeeklyDigest,
		subject: (p) =>
			`Your simse week: ${p.sessions} sessions, ${p.tokensUsed} tokens used`,
	},
	'feature-announcement': {
		component: FeatureAnnouncement,
		subject: (p) => p.title,
	},
	're-engagement': {
		component: ReEngagement,
		subject: (p) => `It's been ${p.daysSinceLogin} days. simse is still here.`,
	},
	'team-invite': {
		component: TeamInvite,
		subject: (p) =>
			`${p.inviterName} invited you to join ${p.teamName} on simse`,
	},
	'role-change': {
		component: RoleChange,
		subject: (p) =>
			`Your role in ${p.teamName} has been updated to ${p.newRole}`,
	},
	'invite-friend': {
		component: InviteFriend,
		subject: (p) => `${p.referrerName} thinks you'd like simse`,
	},
};
