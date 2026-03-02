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

interface FreeCreditEmailProps {
	creditAmount: string;
	dashboardUrl: string;
	expiresIn: string;
	unsubscribeUrl: string;
}

export default function FreeCreditEmail({
	creditAmount,
	dashboardUrl,
	expiresIn,
	unsubscribeUrl,
}: FreeCreditEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>You've got {creditAmount} in free simse credit.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					{/* Emerald accent strip */}
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						{/* Header */}
						<Text className="text-center font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-subtle">
							SIMSE
						</Text>

						{/* Heading */}
						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Free <span className="text-emerald">credit</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							A little something to get you going.
						</Text>

						{/* Credit amount */}
						<Section className="mt-10 text-center">
							<table
								role="presentation"
								cellPadding="0"
								cellSpacing="0"
								border={0}
								style={{ margin: '0 auto' }}
							>
								<tr>
									<td className="rounded-xl border-2 border-emerald bg-card px-12 py-7">
										<Text className="m-0 font-mono text-[42px] font-bold tracking-tight text-emerald">
											{creditAmount}
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Text className="mt-6 text-center text-[13px] leading-relaxed text-dim">
							Added to your account. Expires in {expiresIn}.
						</Text>

						{/* Body copy */}
						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							Use it on any model, any session. No strings attached. We'd rather
							you try everything and decide what's worth paying for.
						</Text>

						{/* CTA Button */}
						<Section className="mt-10 text-center">
							<Button
								href={dashboardUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Open dashboard
							</Button>
						</Section>

						{/* Emerald divider */}
						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* What counts */}
						<Text className="mt-12 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
							What counts toward credit
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
												AI sessions
											</span>{' '}
											&mdash; conversations, tool calls, and agentic loops
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
												Embeddings
											</span>{' '}
											&mdash; library indexing and semantic search
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
											<span className="font-semibold text-bright">Storage</span>{' '}
											&mdash; persistent context and file-backed memory
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						{/* Divider */}
						<Hr className="mt-16 border-card" />

						{/* Footer */}
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

FreeCreditEmail.PreviewProps = {
	creditAmount: '$10',
	dashboardUrl: 'https://app.simse.dev/dashboard',
	expiresIn: '30 days',
	unsubscribeUrl: '#',
} satisfies FreeCreditEmailProps;
