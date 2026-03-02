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
} from '@react-email/components';
import { emailTailwindConfig } from './tailwind-config';

interface VerifyEmailProps {
	verificationCode: string;
	verificationUrl: string;
}

export default function VerifyEmail({
	verificationCode,
	verificationUrl,
}: VerifyEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>Your simse verification code: {verificationCode}</Preview>
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
							Verify your <span className="text-emerald">email</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							Enter this code to confirm your account.
						</Text>

						{/* Verification code */}
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
											{verificationCode}
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Text className="mt-6 text-center text-[13px] leading-relaxed text-dim">
							This code expires in 15 minutes.
						</Text>

						{/* Emerald divider */}
						<Section className="mt-10 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* Or use link */}
						<Text className="mt-10 text-center text-[14px] leading-relaxed text-body">
							Or verify directly:{' '}
							<Link href={verificationUrl} className="text-emerald underline">
								click here
							</Link>
						</Text>

						{/* Security note */}
						<Section className="mt-12 rounded-xl bg-card p-7">
							<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
								Didn't request this?
							</Text>
							<Text className="m-0 mt-4 text-[14px] leading-[1.75] text-body">
								If you didn't create a simse account, you can safely ignore this
								email. The code will expire on its own and no account will be
								created.
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

VerifyEmail.PreviewProps = {
	verificationCode: '847 291',
	verificationUrl: 'https://app.simse.dev/verify?code=847291',
} satisfies VerifyEmailProps;
