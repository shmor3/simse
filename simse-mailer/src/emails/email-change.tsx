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

interface EmailChangeEmailProps {
	newEmail: string;
	confirmUrl: string;
}

export default function EmailChangeEmail({
	newEmail,
	confirmUrl,
}: EmailChangeEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>Confirm your new simse email address.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<SimseEmailLogo />

						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							Confirm your <span className="text-emerald">email</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							You requested to change your email address.
						</Text>

						{/* New email display */}
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
										<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
											New email
										</Text>
										<Text className="m-0 mt-2 text-[18px] font-semibold text-white">
											{newEmail}
										</Text>
									</td>
								</tr>
							</table>
						</Section>

						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							Click the button below to confirm this change. Your old email will
							remain active until you confirm.
						</Text>

						<Section className="mt-10 text-center">
							<Button
								href={confirmUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Confirm email change
							</Button>
						</Section>

						<Text className="mt-5 text-center text-[13px] leading-relaxed text-dim">
							This link expires in 24 hours.
						</Text>

						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						<Section className="mt-12 rounded-xl bg-card p-7">
							<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
								Didn't request this?
							</Text>
							<Text className="m-0 mt-4 text-[14px] leading-[1.75] text-body">
								If you didn't request an email change, your account may be
								compromised. Reply to this email immediately and we'll help
								secure your account.
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

EmailChangeEmail.PreviewProps = {
	newEmail: 'alex@newdomain.com',
	confirmUrl: 'https://app.simse.dev/confirm-email?token=abc123',
} satisfies EmailChangeEmailProps;
