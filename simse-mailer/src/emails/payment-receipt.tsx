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

interface PaymentReceiptEmailProps {
	amount: string;
	planName: string;
	billingDate: string;
	invoiceUrl: string;
	unsubscribeUrl: string;
}

export default function PaymentReceiptEmail({
	amount,
	planName,
	billingDate,
	invoiceUrl,
	unsubscribeUrl,
}: PaymentReceiptEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>
					Receipt for your simse {planName} plan &mdash; {amount}
				</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<SimseEmailLogo />

						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Payment <span className="text-emerald">received</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							Thanks for your continued support.
						</Text>

						{/* Receipt card */}
						<Section className="mt-10 rounded-xl border-2 border-border bg-card p-7">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td>
										<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
											Amount
										</Text>
									</td>
									<td align="right">
										<Text className="m-0 font-mono text-[22px] font-bold text-emerald">
											{amount}
										</Text>
									</td>
								</tr>
							</table>
							<Hr className="my-5 border-border" />
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td>
										<Text className="m-0 text-[13px] text-muted">Plan</Text>
									</td>
									<td align="right">
										<Text className="m-0 text-[13px] font-semibold text-bright">
											{planName}
										</Text>
									</td>
								</tr>
							</table>
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td>
										<Text className="m-0 text-[13px] text-muted">Date</Text>
									</td>
									<td align="right">
										<Text className="m-0 text-[13px] font-semibold text-bright">
											{billingDate}
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Section className="mt-10 text-center">
							<Button
								href={invoiceUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								View invoice
							</Button>
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

PaymentReceiptEmail.PreviewProps = {
	amount: '$24',
	planName: 'Pro',
	billingDate: 'Mar 1, 2026',
	invoiceUrl: 'https://app.simse.dev/billing/inv_abc123',
	unsubscribeUrl: '#',
} satisfies PaymentReceiptEmailProps;
