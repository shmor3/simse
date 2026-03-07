import {
	Body,
	Button,
	Container,
	Head,
	Heading,
	Hr,
	Html,
	Link,
	Preview,
	Section,
	Tailwind,
	Text,
} from '@react-email/components';
import SimseEmailLogo from './simse-logo';
import { emailTailwindConfig } from './tailwind-config';

interface PaymentFailedEmailProps {
	amount: string;
	retryDate: string;
	updateUrl: string;
	unsubscribeUrl: string;
}

export default function PaymentFailedEmail({
	amount,
	retryDate,
	updateUrl,
	unsubscribeUrl,
}: PaymentFailedEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>Your simse payment of {amount} didn't go through.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<SimseEmailLogo />

						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Payment <span className="text-emerald">failed</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							We couldn't process your latest payment.
						</Text>

						{/* Amount card */}
						<Section className="mt-10 text-center">
							<table
								role="presentation"
								cellPadding="0"
								cellSpacing="0"
								border={0}
								style={{ margin: '0 auto' }}
							>
								<tr>
									<td className="rounded-xl border-2 border-border bg-card px-10 py-6">
										<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
											Amount due
										</Text>
										<Text className="m-0 mt-2 font-mono text-[28px] font-bold text-white">
											{amount}
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							Please update your payment method to keep your account active.
							We'll retry automatically on{' '}
							<span className="font-semibold text-bright">{retryDate}</span>.
						</Text>

						<Section className="mt-10 text-center">
							<Button
								href={updateUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Update payment
							</Button>
						</Section>

						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						<Section className="mt-12 rounded-xl bg-card p-7">
							<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
								What happens next
							</Text>
							<Text className="m-0 mt-4 text-[14px] leading-[1.75] text-body">
								Your account stays fully active during the grace period. If the
								retry fails, your plan will be paused&mdash;but your data and
								settings will be preserved. Update your payment method anytime
								to resume.
							</Text>
						</Section>

						<Hr className="mt-16 border-card" />

						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You're receiving this because you have an active simse
							subscription.{' '}
							<Link href={unsubscribeUrl} className="text-dim underline">
								Unsubscribe
							</Link>
						</Text>
						<Text className="mt-3 text-center font-mono text-[11px] text-border">
							&copy; 2026 simse
						</Text>
					</Container>
				</Body>
			</Tailwind>
		</Html>
	);
}

PaymentFailedEmail.PreviewProps = {
	amount: '$24',
	retryDate: 'Mar 4, 2026',
	updateUrl: 'https://app.simse.dev/billing',
	unsubscribeUrl: '#',
} satisfies PaymentFailedEmailProps;
