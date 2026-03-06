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

interface OnboardingEmailProps {
	dashboardUrl: string;
	unsubscribeUrl: string;
}

export default function OnboardingEmail({
	dashboardUrl,
	unsubscribeUrl,
}: OnboardingEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>Three things to try in your first simse session.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<SimseEmailLogo />

						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Getting <span className="text-emerald">started</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							Three things worth trying in your first session.
						</Text>

						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							simse gets better the more you use it. Here's how to make the most
							of your first few minutes.
						</Text>

						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* Step 1 */}
						<Section className="mt-12">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="32" valign="top" style={{ paddingTop: 2 }}>
										<Text className="m-0 font-mono text-[14px] font-bold text-emerald">
											01
										</Text>
									</td>
									<td style={{ paddingLeft: 14 }}>
										<Text className="m-0 text-[15px] font-semibold text-bright">
											Have a real conversation
										</Text>
										<Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
											Don't test it&mdash;use it. Ask something you'd normally
											search for. Then follow up. Context carries forward
											automatically.
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Step 2 */}
						<Section className="mt-6">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="32" valign="top" style={{ paddingTop: 2 }}>
										<Text className="m-0 font-mono text-[14px] font-bold text-emerald">
											02
										</Text>
									</td>
									<td style={{ paddingLeft: 14 }}>
										<Text className="m-0 text-[15px] font-semibold text-bright">
											Connect a tool
										</Text>
										<Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
											Go to Settings &rarr; Tools and add an MCP server. It'll
											surface in your workflow automatically&mdash;no config
											needed per session.
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Step 3 */}
						<Section className="mt-6">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="32" valign="top" style={{ paddingTop: 2 }}>
										<Text className="m-0 font-mono text-[14px] font-bold text-emerald">
											03
										</Text>
									</td>
									<td style={{ paddingLeft: 14 }}>
										<Text className="m-0 text-[15px] font-semibold text-bright">
											Come back tomorrow
										</Text>
										<Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
											The real value shows up on session two. simse remembers
											what you discussed, what tools you used, and what you
											prefer.
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Section className="mt-10 text-center">
							<Button
								href={dashboardUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Open simse
							</Button>
						</Section>

						<Section className="mt-14 rounded-xl bg-card p-7">
							<Text className="m-0 text-[14px] leading-[1.75] text-body">
								Stuck? Have feedback? Just reply to this email. It reaches a
								real person, and we read every message.
							</Text>
							<Text className="m-0 mt-5 text-[14px] text-dim">
								&mdash; The simse team
							</Text>
						</Section>

						<Hr className="mt-16 border-card" />

						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You're receiving this because you just created a simse account.{' '}
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

OnboardingEmail.PreviewProps = {
	dashboardUrl: 'https://app.simse.dev',
	unsubscribeUrl: '#',
} satisfies OnboardingEmailProps;
