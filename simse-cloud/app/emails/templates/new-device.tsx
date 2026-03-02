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
import SimseEmailLogo from './simse-logo';

interface NewDeviceEmailProps {
	device: string;
	location: string;
	time: string;
	secureUrl: string;
}

export default function NewDeviceEmail({
	device,
	location,
	time,
	secureUrl,
}: NewDeviceEmailProps) {
	return (
		<Html lang="en">
			<Tailwind config={emailTailwindConfig}>
				<Head>
					<meta name="color-scheme" content="dark" />
					<meta name="supported-color-schemes" content="dark" />
				</Head>
				<Preview>New sign-in to your simse account from {device}.</Preview>
				<Body className="m-0 bg-surface font-sans antialiased">
					<div className="mx-auto h-1 max-w-[500px] bg-emerald" />

					<Container className="mx-auto max-w-[500px] px-6 pb-14 pt-10">
						<SimseEmailLogo />

						<Heading className="mt-12 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
							New <span className="text-emerald">sign-in</span>.
						</Heading>
						<Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
							We noticed a login from a new device.
						</Text>

						{/* Device details */}
						<Section className="mt-10 rounded-xl border-2 border-border bg-card p-7">
							<Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
								Login details
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
										<Text className="m-0 text-[13px] text-muted">Device</Text>
									</td>
									<td align="right">
										<Text className="m-0 text-[13px] font-semibold text-bright">
											{device}
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
							If this was you, no action is needed. If you don't recognize this
							login, secure your account immediately.
						</Text>

						<Section className="mt-10 text-center">
							<Button
								href={secureUrl}
								className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
							>
								Secure my account
							</Button>
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

NewDeviceEmail.PreviewProps = {
	device: 'Chrome on macOS',
	location: 'Copenhagen, Denmark',
	time: 'Mar 1, 2026 at 14:32 UTC',
	secureUrl: 'https://app.simse.dev/security',
} satisfies NewDeviceEmailProps;
