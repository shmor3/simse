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
import { emailTailwindConfig } from './tailwind-config';
import SimseEmailLogo from './simse-logo';

interface UsageWarningEmailProps {
	usedPercent: number;
	currentUsage: string;
	limit: string;
	upgradeUrl: string;
	unsubscribeUrl: string;
}

export default function UsageWarningEmail({
	usedPercent,
	currentUsage,
	limit,
	upgradeUrl,
	unsubscribeUrl,
}: UsageWarningEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>
					You've used {String(usedPercent)}% of your simse credit this cycle.
				</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<SimseEmailLogo />

						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Usage <span className="text-emerald">alert</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							You're approaching your credit limit.
						</Text>

						{/* Usage meter */}
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
											Credit used
										</Text>
									</td>
									<td align="right">
										<Text className="m-0 font-mono text-[22px] font-bold text-emerald">
											{usedPercent}%
										</Text>
									</td>
								</tr>
							</table>

							{/* Progress bar */}
							<div className="mt-4 h-2 w-full rounded-full bg-border">
								<div
									className="h-2 rounded-full bg-emerald"
									style={{ width: `${usedPercent}%` }}
								/>
							</div>

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
										<Text className="m-0 text-[13px] text-muted">Used</Text>
									</td>
									<td align="right">
										<Text className="m-0 text-[13px] font-semibold text-bright">
											{currentUsage}
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
										<Text className="m-0 text-[13px] text-muted">Limit</Text>
									</td>
									<td align="right">
										<Text className="m-0 text-[13px] font-semibold text-bright">
											{limit}
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							Once you hit your limit, sessions will pause until your next
							billing cycle. Upgrade now to keep going without interruption.
						</Text>

						<Section className="mt-10 text-center">
							<Button
								href={upgradeUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Upgrade plan
							</Button>
						</Section>

						<Hr className="mt-16 border-card" />

						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You're receiving this because you enabled usage alerts.{' '}
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

UsageWarningEmail.PreviewProps = {
	usedPercent: 85,
	currentUsage: '$20.40',
	limit: '$24.00',
	upgradeUrl: 'https://app.simse.dev/billing/upgrade',
	unsubscribeUrl: '#',
} satisfies UsageWarningEmailProps;
