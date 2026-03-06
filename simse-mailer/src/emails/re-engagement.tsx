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

interface ReEngagementEmailProps {
	daysSinceLogin: number;
	dashboardUrl: string;
	unsubscribeUrl: string;
}

export default function ReEngagementEmail({
	daysSinceLogin,
	dashboardUrl,
	unsubscribeUrl,
}: ReEngagementEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>
					It's been {String(daysSinceLogin)} days. simse is still here.
				</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<SimseEmailLogo />

						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Still <span className="text-emerald">here</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							It's been {daysSinceLogin} days since your last session.
						</Text>

						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							Your context is preserved. Your tools are connected. Your library
							is waiting. Pick up right where you left off&mdash;simse
							remembers.
						</Text>

						<Section className="mt-10 text-center">
							<Button
								href={dashboardUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Resume session
							</Button>
						</Section>

						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* What's changed */}
						<Text className="mt-12 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
							Since you've been away
						</Text>

						<Section className="mt-8">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="28" valign="top" style={{ paddingTop: 5 }}>
										<div className="h-2.5 w-2.5 rounded-full bg-emerald" />
									</td>
									<td style={{ paddingLeft: 12 }}>
										<Text className="m-0 text-[15px] leading-[1.75] text-body">
											<span className="font-semibold text-bright">
												Faster sessions
											</span>{' '}
											&mdash; reduced latency across all models
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Section className="mt-4">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="28" valign="top" style={{ paddingTop: 5 }}>
										<div className="h-2.5 w-2.5 rounded-full bg-emerald" />
									</td>
									<td style={{ paddingLeft: 12 }}>
										<Text className="m-0 text-[15px] leading-[1.75] text-body">
											<span className="font-semibold text-bright">
												New tool integrations
											</span>{' '}
											&mdash; more MCP servers supported out of the box
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Section className="mt-4">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="28" valign="top" style={{ paddingTop: 5 }}>
										<div className="h-2.5 w-2.5 rounded-full bg-emerald" />
									</td>
									<td style={{ paddingLeft: 12 }}>
										<Text className="m-0 text-[15px] leading-[1.75] text-body">
											<span className="font-semibold text-bright">
												Smarter memory
											</span>{' '}
											&mdash; improved context carry-over between sessions
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Hr className="mt-16 border-card" />

						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You're receiving this because you have a simse account.{' '}
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

ReEngagementEmail.PreviewProps = {
	daysSinceLogin: 14,
	dashboardUrl: 'https://app.simse.dev',
	unsubscribeUrl: '#',
} satisfies ReEngagementEmailProps;
