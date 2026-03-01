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
	pixelBasedPreset,
} from '@react-email/components';

interface WelcomeEmailProps {
	unsubscribeUrl: string;
}

export default function WelcomeEmail({ unsubscribeUrl }: WelcomeEmailProps) {
	return (
		<Html lang="en">
			<Tailwind
				config={{
					presets: [pixelBasedPreset],
					theme: {
						extend: {
							colors: {
								surface: '#0a0a0b',
								card: '#18181b',
								border: '#27272a',
								emerald: '#34d399',
								muted: '#71717a',
								subtle: '#3f3f46',
								dim: '#52525b',
								body: '#a1a1aa',
								light: '#d4d4d8',
								bright: '#e4e4e7',
							},
							fontFamily: {
								mono: [
									'Courier New',
									'Courier',
									'monospace',
								],
							},
						},
					},
				}}
			>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta
						name="supported-color-schemes"
						content="dark"
					/>
				</Head>
				<Preview>Your spot on the simse waitlist is confirmed.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<Container className="mx-auto max-w-[500px] px-5 py-14">
						{/* Header */}
						<Text className="text-center font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-subtle">
							SIMSE-CODE
						</Text>

						{/* Heading */}
						<Heading className="mt-10 text-center text-4xl font-bold leading-tight tracking-tight text-white">
							You're <span className="text-emerald">in</span>.
						</Heading>
						<Text className="mt-1 text-center text-base leading-relaxed text-muted">
							Your spot on the waitlist is confirmed.
						</Text>

						{/* Body copy */}
						<Text className="mt-10 text-center text-[15px] leading-[1.8] text-body">
							We're building simse to be the assistant you
							don't have to start over with every session. It
							remembers your context, learns your preferences,
							and connects to whatever AI tools you already
							use. The more you work with it, the less you
							have to repeat yourself.
						</Text>

						{/* Emerald divider */}
						<Section className="mt-10 text-center">
							<div className="mx-auto h-0.5 w-10 rounded-sm bg-emerald" />
						</Section>

						{/* Roadmap heading */}
						<Text className="mt-10 font-mono text-[10px] uppercase tracking-[0.2em] text-dim">
							The road from here
						</Text>

						{/* Step 1 — completed */}
						<Section className="mt-6">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="36" valign="top" style={{ paddingTop: 1 }}>
										<div className="h-6 w-6 rounded-full bg-emerald text-center font-mono text-[11px] font-bold leading-6 text-surface">
											&#10003;
										</div>
									</td>
									<td style={{ paddingLeft: 12 }}>
										<Text className="m-0 text-sm font-semibold text-bright">
											Waitlist confirmed
										</Text>
										<Text className="m-0 mt-0.5 text-[13px] leading-relaxed text-muted">
											You're on the list. No action needed.
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Connector */}
						<div className="ml-[11px] mt-4 h-5 w-0.5 rounded-sm bg-border" />

						{/* Step 2 */}
						<Section className="mt-4">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="36" valign="top" style={{ paddingTop: 1 }}>
										<div className="h-6 w-6 rounded-full border-2 border-border text-center font-mono text-[11px] font-bold leading-5 text-dim">
											2
										</div>
									</td>
									<td style={{ paddingLeft: 12 }}>
										<Text className="m-0 text-sm font-semibold text-bright">
											Early preview
										</Text>
										<Text className="m-0 mt-0.5 text-[13px] leading-relaxed text-muted">
											We'll share a first look at simse before it goes
											public so you can kick the tires.
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Connector */}
						<div className="ml-[11px] mt-4 h-5 w-0.5 rounded-sm bg-border" />

						{/* Step 3 */}
						<Section className="mt-4">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td width="36" valign="top" style={{ paddingTop: 1 }}>
										<div className="h-6 w-6 rounded-full border-2 border-border text-center font-mono text-[11px] font-bold leading-5 text-dim">
											3
										</div>
									</td>
									<td style={{ paddingLeft: 12 }}>
										<Text className="m-0 text-sm font-semibold text-bright">
											Your invite
										</Text>
										<Text className="m-0 mt-0.5 text-[13px] leading-relaxed text-muted">
											When early access opens, you'll get in first.
											We'll email you a direct link&mdash;no waitlist
											shuffle.
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Divider */}
						<Hr className="mt-14 border-card" />

						{/* Footer */}
						<Text className="mt-8 text-center text-[11px] leading-relaxed text-subtle">
							You signed up at simse.dev.{' '}
							<Link
								href={unsubscribeUrl}
								className="text-dim underline"
							>
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
