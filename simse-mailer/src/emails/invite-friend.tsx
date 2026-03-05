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

interface InviteFriendEmailProps {
	referrerName: string;
	inviteUrl: string;
	unsubscribeUrl: string;
}

export default function InviteFriendEmail({
	referrerName,
	inviteUrl,
	unsubscribeUrl,
}: InviteFriendEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>{referrerName} thinks you'd like simse.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					{/* Emerald accent strip */}
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						{/* Header */}
						<SimseEmailLogo />

						{/* Referral badge */}
						<Section className="mt-12 text-center">
							<table
								role="presentation"
								cellPadding="0"
								cellSpacing="0"
								border={0}
								style={{ margin: '0 auto' }}
							>
								<tr>
									<td className="rounded-full border border-border px-4 py-1.5">
										<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.2em] text-dim">
											Invited by{' '}
											<span className="text-emerald">{referrerName}</span>
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Heading */}
						<Heading className="mt-10 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							You've been <span className="text-emerald">picked</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							Someone you know thinks simse is worth your time.
						</Text>

						{/* Body copy */}
						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							simse is an assistant that doesn't start from scratch every
							session. It remembers your context, learns your preferences, and
							connects to your existing tools. {referrerName} is already using
							it&mdash;and thought you'd want in too.
						</Text>

						{/* CTA Button */}
						<Section className="mt-10 text-center">
							<Button
								href={inviteUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Accept invite
							</Button>
						</Section>

						<Text className="mt-5 text-center text-[13px] leading-relaxed text-dim">
							This invite link is personal to you.
						</Text>

						{/* Emerald divider */}
						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* What you'll get */}
						<Text className="mt-12 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
							What you'll get
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
												Context that persists
											</span>{' '}
											&mdash; pick up where you left off, every time
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
												Your tools, connected
											</span>{' '}
											&mdash; plug in any MCP or ACP provider instantly
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
												Early access
											</span>{' '}
											&mdash; full access, no feature gates, no waitlist
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Divider */}
						<Hr className="mt-16 border-card" />

						{/* Footer */}
						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You received this because {referrerName} invited you.{' '}
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

InviteFriendEmail.PreviewProps = {
	referrerName: 'Alex',
	inviteUrl: 'https://app.simse.dev/invite/ref-abc123',
	unsubscribeUrl: '#',
} satisfies InviteFriendEmailProps;
