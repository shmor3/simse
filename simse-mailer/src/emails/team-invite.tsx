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

interface TeamInviteEmailProps {
	inviterName: string;
	teamName: string;
	inviteUrl: string;
	unsubscribeUrl: string;
}

export default function TeamInviteEmail({
	inviterName,
	teamName,
	inviteUrl,
	unsubscribeUrl,
}: TeamInviteEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>
					{inviterName} invited you to join {teamName} on simse.
				</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<SimseEmailLogo />

						{/* Team badge */}
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
											Team <span className="text-emerald">{teamName}</span>
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Heading className="mt-10 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Join the <span className="text-emerald">team</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							{inviterName} wants you on board.
						</Text>

						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							You've been invited to join the{' '}
							<span className="font-semibold text-bright">{teamName}</span>{' '}
							workspace on simse. You'll share tools, configurations, and
							billing under one account.
						</Text>

						<Section className="mt-10 text-center">
							<Button
								href={inviteUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Join team
							</Button>
						</Section>

						<Text className="mt-5 text-center text-[13px] leading-relaxed text-dim">
							This invite expires in 7 days.
						</Text>

						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* What you get */}
						<Text className="mt-12 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
							As a team member
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
												Shared tools
											</span>{' '}
											&mdash; MCP and ACP providers configured once for everyone
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
												Unified billing
											</span>{' '}
											&mdash; one subscription, managed by the team admin
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
												Private sessions
											</span>{' '}
											&mdash; your conversations stay yours, only tools are
											shared
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Hr className="mt-16 border-card" />

						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You received this because {inviterName} invited you.{' '}
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

TeamInviteEmail.PreviewProps = {
	inviterName: 'Alex',
	teamName: 'Acme Labs',
	inviteUrl: 'https://app.simse.dev/team/invite/abc123',
	unsubscribeUrl: '#',
} satisfies TeamInviteEmailProps;
