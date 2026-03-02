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

interface WeeklyDigestEmailProps {
	weekOf: string;
	sessions: number;
	tokensUsed: string;
	libraryItems: number;
	dashboardUrl: string;
	unsubscribeUrl: string;
}

export default function WeeklyDigestEmail({
	weekOf,
	sessions,
	tokensUsed,
	libraryItems,
	dashboardUrl,
	unsubscribeUrl,
}: WeeklyDigestEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>
					Your simse week: {String(sessions)} sessions, {tokensUsed} tokens
					used.
				</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<Text className="text-center font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-subtle">
							SIMSE
						</Text>

						{/* Week badge */}
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
											Week of <span className="text-emerald">{weekOf}</span>
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Heading className="mt-10 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Your <span className="text-emerald">week</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							Here's what you accomplished with simse.
						</Text>

						{/* Stats grid */}
						<Section className="mt-10">
							<table
								role="presentation"
								width="100%"
								cellPadding="0"
								cellSpacing="0"
								border={0}
							>
								<tr>
									<td
										width="33%"
										align="center"
										className="rounded-l-xl border-2 border-r border-border bg-card py-6"
									>
										<Text className="m-0 font-mono text-[24px] font-bold text-emerald">
											{sessions}
										</Text>
										<Text className="m-0 mt-1 font-mono text-[9px] uppercase tracking-[0.2em] text-dim">
											Sessions
										</Text>
									</td>
									<td
										width="34%"
										align="center"
										className="border-2 border-x border-border bg-card py-6"
									>
										<Text className="m-0 font-mono text-[24px] font-bold text-emerald">
											{tokensUsed}
										</Text>
										<Text className="m-0 mt-1 font-mono text-[9px] uppercase tracking-[0.2em] text-dim">
											Tokens
										</Text>
									</td>
									<td
										width="33%"
										align="center"
										className="rounded-r-xl border-2 border-l border-border bg-card py-6"
									>
										<Text className="m-0 font-mono text-[24px] font-bold text-emerald">
											{libraryItems}
										</Text>
										<Text className="m-0 mt-1 font-mono text-[9px] uppercase tracking-[0.2em] text-dim">
											Library
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							Every session teaches simse a little more about how you work. Your
							library is growing, and context is getting sharper.
						</Text>

						<Section className="mt-10 text-center">
							<Button
								href={dashboardUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								View dashboard
							</Button>
						</Section>

						<Hr className="mt-16 border-card" />

						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You're receiving this weekly summary.{' '}
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

WeeklyDigestEmail.PreviewProps = {
	weekOf: 'Feb 24',
	sessions: 23,
	tokensUsed: '142k',
	libraryItems: 47,
	dashboardUrl: 'https://app.simse.dev/dashboard',
	unsubscribeUrl: '#',
} satisfies WeeklyDigestEmailProps;
