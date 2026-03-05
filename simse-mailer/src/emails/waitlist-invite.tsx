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

interface InviteEmailProps {
	inviteUrl: string;
	unsubscribeUrl: string;
}

export default function InviteEmail({
	inviteUrl,
	unsubscribeUrl,
}: InviteEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>Your simse early access is ready.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					{/* Emerald accent strip */}
					<div className="mx-auto h-1 max-w-125 bg-emerald" />

					<Container className="mx-auto max-w-125 px-6 pb-14 pt-10">
						{/* Header */}
						<SimseEmailLogo />

						{/* Progress indicator — all complete */}
						<Section className="mt-12 text-center">
							<table
								role="presentation"
								cellPadding="0"
								cellSpacing="0"
								border={0}
								style={{ margin: '0 auto' }}
							>
								<tr>
									<td style={{ padding: '0 3px' }}>
										<div className="h-1.5 w-10 rounded-full bg-emerald" />
									</td>
									<td style={{ padding: '0 3px' }}>
										<div className="h-1.5 w-10 rounded-full bg-emerald" />
									</td>
									<td style={{ padding: '0 3px' }}>
										<div className="h-1.5 w-10 rounded-full bg-emerald" />
									</td>
								</tr>
							</table>
							<Text className="mt-3 font-mono text-[10px] uppercase tracking-[0.2em] text-emerald">
								You're in
							</Text>
						</Section>

						{/* Heading */}
						<Heading className="mt-10 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							It's <span className="text-emerald">yours</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							Your early access to simse is ready.
						</Text>

						{/* Body copy */}
						<Text className="mx-auto mt-10 max-w-105 text-center text-[15px] leading-[1.85] text-body">
							You waited. You tested the preview. You told us what was broken.
							This is the result&mdash;a better version of simse, shaped by the
							people who'll actually use it. That includes you.
						</Text>

						{/* CTA Button */}
						<Section className="mt-10 text-center">
							<Button
								href={inviteUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Open simse
							</Button>
						</Section>

						<Text className="mt-5 text-center text-[13px] leading-relaxed text-dim">
							This link is yours. It won't expire.
						</Text>

						{/* Emerald divider */}
						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* What you get */}
						<Text className="mt-12 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
							What you get
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
												Full access
											</span>{' '}
											&mdash; everything we've built, no feature gates
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
												Persistent context
											</span>{' '}
											&mdash; sessions that remember across conversations
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
												Your tool stack
											</span>{' '}
											&mdash; connect any ACP or MCP provider out of the box
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
												Direct line
											</span>{' '}
											&mdash; reply to any email from us and it reaches a human
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Closing note */}
						<Section className="mt-14 rounded-xl bg-card p-7">
							<Text className="m-0 text-[14px] leading-[1.75] text-body">
								We built simse because we were tired of assistants that forget
								everything between sessions. If you feel the same way, we think
								you'll like what's here. And it's only going to get better.
							</Text>
							<Text className="m-0 mt-5 text-[14px] text-dim">
								&mdash; The simse team
							</Text>
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

InviteEmail.PreviewProps = {
	inviteUrl: 'https://app.simse.dev/invite/abc123',
	unsubscribeUrl: '#',
} satisfies InviteEmailProps;
