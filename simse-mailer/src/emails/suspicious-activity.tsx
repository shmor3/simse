import {
	Body,
	Button,
	Container,
	Head,
	Heading,
	Hr,
	Html,
	Preview,
	Section,
	Tailwind,
	Text,
} from '@react-email/components';
import SimseEmailLogo from './simse-logo';
import { emailTailwindConfig } from './tailwind-config';

interface SuspiciousActivityEmailProps {
	activity: string;
	location: string;
	time: string;
	secureUrl: string;
}

export default function SuspiciousActivityEmail({
	activity,
	location,
	time,
	secureUrl,
}: SuspiciousActivityEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>Unusual activity detected on your simse account.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<SimseEmailLogo />

						{/* Alert badge */}
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
											Security alert
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Heading className="mt-10 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Unusual <span className="text-emerald">activity</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							We detected something out of the ordinary.
						</Text>

						{/* Activity details */}
						<Section className="mt-10 rounded-xl border-2 border-border bg-card p-7">
							<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
								What we detected
							</Text>

							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
								className="mt-5"
							>
								<tr>
									<td>
										<Text className="m-0 text-[13px] text-muted">Activity</Text>
									</td>
									<td align="right">
										<Text className="m-0 text-[13px] font-semibold text-bright">
											{activity}
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
										<Text className="m-0 text-[13px] text-muted">Location</Text>
									</td>
									<td align="right">
										<Text className="m-0 text-[13px] font-semibold text-bright">
											{location}
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
										<Text className="m-0 text-[13px] text-muted">Time</Text>
									</td>
									<td align="right">
										<Text className="m-0 text-[13px] font-semibold text-bright">
											{time}
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							We temporarily blocked this action as a precaution. If this was
							you, no further action is needed. Otherwise, secure your account
							now.
						</Text>

						<Section className="mt-10 text-center">
							<Button
								href={secureUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Secure my account
							</Button>
						</Section>

						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						<Section className="mt-12 rounded-xl bg-card p-7">
							<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
								Recommended steps
							</Text>
							<Text className="m-0 mt-4 text-[14px] leading-[1.75] text-body">
								Change your password, review active sessions, and enable
								two-factor authentication if you haven't already. Reply to this
								email if you need help.
							</Text>
						</Section>

						<Hr className="mt-16 border-card" />

						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							simse.dev
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

SuspiciousActivityEmail.PreviewProps = {
	activity: 'Multiple failed login attempts',
	location: 'Unknown (VPN detected)',
	time: 'Mar 1, 2026 at 03:17 UTC',
	secureUrl: 'https://app.simse.dev/security',
} satisfies SuspiciousActivityEmailProps;
