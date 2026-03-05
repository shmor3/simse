import {
	Body,
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

interface WelcomeEmailProps {
	unsubscribeUrl: string;
}

export default function WelcomeEmail({ unsubscribeUrl }: WelcomeEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>Your spot on the simse waitlist is confirmed.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					{/* Emerald accent strip */}
					<div className="mx-auto h-1 max-w-125 bg-emerald" />

					<Container className="mx-auto max-w-125 px-6 pb-14 pt-10">
						{/* Header */}
						<SimseEmailLogo />

						{/* Heading */}
						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							You're <span className="text-emerald">in</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							Your spot on the waitlist is confirmed.
						</Text>

						{/* Body copy */}
						<Text className="mx-auto mt-10 max-w-105 text-center text-[15px] leading-[1.85] text-body">
							We're building simse to be the assistant you don't have to start
							over with every session. It remembers your context, learns your
							preferences, and connects to whatever tools you already use.
						</Text>

						{/* Emerald divider */}
						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* Roadmap heading */}
						<Text className="mt-12 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
							The road from here
						</Text>

						{/* Step 1 — completed */}
						<Section className="mt-8">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="40" valign="top">
										<div
											style={{
												width: 28,
												height: 28,
												borderRadius: '50%',
												backgroundColor: '#34d399',
												textAlign: 'center',
												fontFamily: 'monospace',
												fontSize: 12,
												fontWeight: 700,
												lineHeight: '28px',
												color: '#0a0a0b',
											}}
										>
											&#10003;
										</div>
									</td>
									<td style={{ paddingLeft: 14 }}>
										<Text className="m-0 text-[15px] font-semibold text-bright">
											Waitlist confirmed
										</Text>
										<Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
											You're on the list. No action needed.
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Connector */}
						<div className="ml-3.25 mt-3 h-6 w-px bg-border" />

						{/* Step 2 */}
						<Section className="mt-3">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="40" valign="top">
										<div
											style={{
												width: 28,
												height: 28,
												borderRadius: '50%',
												border: '2px solid #27272a',
												textAlign: 'center',
												fontFamily: 'monospace',
												fontSize: 12,
												fontWeight: 700,
												lineHeight: '24px',
												color: '#52525b',
											}}
										>
											2
										</div>
									</td>
									<td style={{ paddingLeft: 14 }}>
										<Text className="m-0 text-[15px] font-semibold text-bright">
											Early preview
										</Text>
										<Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
											We'll share a first look before it goes public so you can
											kick the tires.
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Connector */}
						<div className="ml-3.25 mt-3 h-6 w-px bg-border" />

						{/* Step 3 */}
						<Section className="mt-3">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="40" valign="top">
										<div
											style={{
												width: 28,
												height: 28,
												borderRadius: '50%',
												border: '2px solid #27272a',
												textAlign: 'center',
												fontFamily: 'monospace',
												fontSize: 12,
												fontWeight: 700,
												lineHeight: '24px',
												color: '#52525b',
											}}
										>
											3
										</div>
									</td>
									<td style={{ paddingLeft: 14 }}>
										<Text className="m-0 text-[15px] font-semibold text-bright">
											Your invite
										</Text>
										<Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
											When early access opens, you'll get in first. We'll email
											you a direct link&mdash;no waitlist shuffle.
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Divider */}
						<Hr className="mt-16 border-card" />

						{/* Footer */}
						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You signed up at simse.dev.{' '}
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

WelcomeEmail.PreviewProps = {
	unsubscribeUrl: '#',
} satisfies WelcomeEmailProps;
