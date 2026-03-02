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

interface Feature {
	title: string;
	description: string;
}

interface FeatureAnnouncementEmailProps {
	title: string;
	summary: string;
	features: Feature[];
	changelogUrl: string;
	unsubscribeUrl: string;
}

export default function FeatureAnnouncementEmail({
	title,
	summary,
	features,
	changelogUrl,
	unsubscribeUrl,
}: FeatureAnnouncementEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>{title}</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<Text className="text-center font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-subtle">
							SIMSE
						</Text>

						{/* Changelog badge */}
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
											What's <span className="text-emerald">new</span>
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Heading className="mt-10 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							{title}
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							{summary}
						</Text>

						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* Feature list */}
						{features.map((feature, i) => (
							<Section
								key={feature.title}
								className={i === 0 ? 'mt-12' : 'mt-6'}
							>
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
											<Text className="m-0 text-[15px] font-semibold text-bright">
												{feature.title}
											</Text>
											<Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
												{feature.description}
											</Text>
										</td>
									</tr>
								</table>
							</Section>
						))}

						<Section className="mt-10 text-center">
							<Button
								href={changelogUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								See full changelog
							</Button>
						</Section>

						<Hr className="mt-16 border-card" />

						<Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
							You're receiving product updates from simse.{' '}
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

FeatureAnnouncementEmail.PreviewProps = {
	title: 'Smarter context.',
	summary: 'A handful of improvements that make simse feel sharper.',
	features: [
		{
			title: 'Auto-compaction',
			description:
				'Long sessions now compress automatically, keeping context relevant without hitting limits.',
		},
		{
			title: 'Tool output truncation',
			description:
				"Large tool responses are capped intelligently so they don't flood your conversation.",
		},
		{
			title: 'Session forking',
			description:
				'Branch a conversation at any point to explore different directions without losing the original.',
		},
	],
	changelogUrl: 'https://simse.dev/changelog',
	unsubscribeUrl: '#',
} satisfies FeatureAnnouncementEmailProps;
