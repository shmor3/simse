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

interface RoleChangeEmailProps {
	teamName: string;
	newRole: string;
	changedBy: string;
	dashboardUrl: string;
	unsubscribeUrl: string;
}

export default function RoleChangeEmail({
	teamName,
	newRole,
	changedBy,
	dashboardUrl,
	unsubscribeUrl,
}: RoleChangeEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>
					Your role in {teamName} has been updated to {newRole}.
				</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<Text className="text-center font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-subtle">
							SIMSE
						</Text>

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
							Role <span className="text-emerald">updated</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							Your permissions have changed.
						</Text>

						{/* Role card */}
						<Section className="mt-10 text-center">
							<table
								role="presentation"
								cellPadding="0"
								cellSpacing="0"
								border={0}
								style={{ margin: '0 auto' }}
							>
								<tr>
									<td className="rounded-xl border-2 border-emerald bg-card px-10 py-6">
										<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
											New role
										</Text>
										<Text className="m-0 mt-2 font-mono text-[24px] font-bold text-emerald">
											{newRole}
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Text className="mt-6 text-center text-[13px] leading-relaxed text-dim">
							Changed by {changedBy}
						</Text>

						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							Your access level in the {teamName} workspace has been updated.
							This may affect what tools and settings you can manage.
						</Text>

						<Section className="mt-10 text-center">
							<Button
								href={dashboardUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								View permissions
							</Button>
						</Section>

						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						<Section className="mt-12 rounded-xl bg-card p-7">
							<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
								Questions?
							</Text>
							<Text className="m-0 mt-4 text-[14px] leading-[1.75] text-body">
								If you didn't expect this change, reach out to your team admin
								or reply to this email. We're here to help.
							</Text>
						</Section>

						<Hr className="mt-16 border-card" />

						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You're a member of {teamName} on simse.{' '}
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

RoleChangeEmail.PreviewProps = {
	teamName: 'Acme Labs',
	newRole: 'Admin',
	changedBy: 'Alex',
	dashboardUrl: 'https://app.simse.dev/team/settings',
	unsubscribeUrl: '#',
} satisfies RoleChangeEmailProps;
