import {
	Body,
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

interface TwoFactorEmailProps {
	code: string;
}

export default function TwoFactorEmail({ code }: TwoFactorEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>Your simse login code: {code}</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					{/* Emerald accent strip */}
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						{/* Header */}
						<SimseEmailLogo />

						{/* Heading */}
						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Login <span className="text-emerald">code</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							Enter this code to complete your sign-in.
						</Text>

						{/* 2FA code */}
						<Section className="mt-10 text-center">
							<table
								role="presentation"
								cellPadding="0"
								cellSpacing="0"
								border={0}
								style={{ margin: '0 auto' }}
							>
								<tr>
									<td className="rounded-xl border-2 border-border bg-card px-10 py-6">
										<Text className="m-0 font-mono text-[36px] font-bold tracking-[0.3em] text-white">
											{code}
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Text className="mt-6 text-center text-[13px] leading-relaxed text-dim">
							This code expires in 10 minutes.
						</Text>

						{/* Emerald divider */}
						<Section className="mt-10 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* Context */}
						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							Someone is trying to sign in to your simse account. If this is
							you, enter the code above. If not, you can safely ignore this
							email.
						</Text>

						{/* Security tip */}
						<Section className="mt-12 rounded-xl bg-card p-7">
							<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
								Security tip
							</Text>
							<Text className="m-0 mt-4 text-[14px] leading-[1.75] text-body">
								Never share this code with anyone. simse will never ask for your
								code via phone, chat, or any channel other than this login
								screen.
							</Text>
						</Section>

						{/* Divider */}
						<Hr className="mt-16 border-card" />

						{/* Footer */}
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

TwoFactorEmail.PreviewProps = {
	code: '439 712',
} satisfies TwoFactorEmailProps;
