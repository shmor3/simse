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
import { emailTailwindConfig } from './tailwind-config';

interface ResetPasswordEmailProps {
	resetUrl: string;
}

export default function ResetPasswordEmail({
	resetUrl,
}: ResetPasswordEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>Reset your simse password.</Preview>
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
							Reset your <span className="text-emerald">password</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							We received a request to reset your password.
						</Text>

						{/* Body copy */}
						<Text className="mx-auto mt-10 max-w-[420px] text-center text-[15px] leading-[1.85] text-body">
							Click the button below to choose a new password. This link will
							expire in 1 hour.
						</Text>

						{/* CTA Button */}
						<Section className="mt-10 text-center">
							<Button
								href={resetUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Reset password
							</Button>
						</Section>

						<Text className="mt-5 text-center text-[13px] leading-relaxed text-dim">
							This link expires in 1 hour.
						</Text>

						{/* Emerald divider */}
						<Section className="mt-12 text-center">
							<div className="mx-auto h-px w-12 bg-emerald" />
						</Section>

						{/* Security note */}
						<Section className="mt-12 rounded-xl bg-card p-7">
							<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
								Didn't request this?
							</Text>
							<Text className="m-0 mt-4 text-[14px] leading-[1.75] text-body">
								If you didn't request a password reset, you can safely ignore
								this email. Your password will remain unchanged and the link
								will expire on its own.
							</Text>
							<Text className="m-0 mt-4 text-[14px] leading-[1.75] text-body">
								If you're concerned about unauthorized access, reply to this
								email and we'll help secure your account.
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

ResetPasswordEmail.PreviewProps = {
	resetUrl: 'https://app.simse.dev/reset-password?token=abc123',
} satisfies ResetPasswordEmailProps;
