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
} from "@react-email/components";
import SimseEmailLogo from "./simse-logo";
import { emailTailwindConfig } from "./tailwind-config";

interface EarlyPreviewEmailProps {
  previewUrl: string;
  unsubscribeUrl: string;
}

export default function EarlyPreviewEmail({
  previewUrl,
  unsubscribeUrl,
}: EarlyPreviewEmailProps) {
  return (
    <Html lang="en">
      <Tailwind config={emailTailwindConfig}>
        <Head>
          <meta name="color-scheme" content="dark" />
          <meta name="supported-color-schemes" content="dark" />
        </Head>
        <Preview>simse is taking shape. Here's your first look.</Preview>
        <Body className="m-0 bg-surface font-sans antialiased">
          {/* Emerald accent strip */}
          <div className="mx-auto h-1 max-w-125 bg-emerald" />

          <Container className="mx-auto max-w-125 px-6 pb-14 pt-10">
            {/* Header */}
            <SimseEmailLogo />

            {/* Progress indicator */}
            <Section className="mt-12 text-center">
              <table
                role="presentation"
                cellPadding="0"
                cellSpacing="0"
                border={0}
                style={{ margin: "0 auto" }}
              >
                <tr>
                  <td style={{ padding: "0 3px" }}>
                    <div className="h-1.5 w-10 rounded-full bg-emerald" />
                  </td>
                  <td style={{ padding: "0 3px" }}>
                    <div className="h-1.5 w-10 rounded-full bg-emerald" />
                  </td>
                  <td style={{ padding: "0 3px" }}>
                    <div className="h-1.5 w-10 rounded-full bg-border" />
                  </td>
                </tr>
              </table>
              <Text className="mt-3 font-mono text-[10px] uppercase tracking-[0.2em] text-dim">
                Step 2 of 3
              </Text>
            </Section>

            {/* Heading */}
            <Heading className="mt-10 text-center text-[40px] font-bold leading-tight tracking-tight text-white">
              First <span className="text-emerald">look</span>.
            </Heading>
            <Text className="mt-2 text-center text-[16px] leading-relaxed text-muted">
              simse is taking shape&mdash;and you get to see it first.
            </Text>

            {/* Body copy */}
            <Text className="mx-auto mt-10 max-w-105 text-center text-[15px] leading-[1.85] text-body">
              This is an early build. Some things will be rough. But the core is
              there: sessions that carry context forward, preferences that
              stick, and a tool layer that connects to your existing stack.
            </Text>

            {/* CTA Button */}
            <Section className="mt-10 text-center">
              <Button
                href={previewUrl}
                className="rounded-lg bg-emerald px-12 py-4 text-center font-mono text-[14px] font-bold text-surface no-underline"
              >
                Try the preview
              </Button>
            </Section>

            <Text className="mt-5 text-center text-[13px] leading-relaxed text-dim">
              This link is unique to you. Please don't share it.
            </Text>

            {/* Emerald divider */}
            <Section className="mt-12 text-center">
              <div className="mx-auto h-px w-12 bg-emerald" />
            </Section>

            {/* What to try */}
            <Text className="mt-12 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
              Worth trying
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
                  <td width="32" valign="top" style={{ paddingTop: 2 }}>
                    <Text className="m-0 font-mono text-[14px] font-bold text-emerald">
                      01
                    </Text>
                  </td>
                  <td style={{ paddingLeft: 14 }}>
                    <Text className="m-0 text-[15px] font-semibold text-bright">
                      Start a session
                    </Text>
                    <Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
                      Ask it something, then ask a follow-up later. Watch
                      context carry over.
                    </Text>
                  </td>
                </tr>
              </table>
            </Section>

            <Section className="mt-6">
              <table
                role="presentation"
                width="100%"
                cellPadding="0"
                cellSpacing="0"
                border={0}
              >
                <tr>
                  <td width="32" valign="top" style={{ paddingTop: 2 }}>
                    <Text className="m-0 font-mono text-[14px] font-bold text-emerald">
                      02
                    </Text>
                  </td>
                  <td style={{ paddingLeft: 14 }}>
                    <Text className="m-0 text-[15px] font-semibold text-bright">
                      Connect a tool
                    </Text>
                    <Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
                      Plug in an MCP server and see it surface in your workflow
                      automatically.
                    </Text>
                  </td>
                </tr>
              </table>
            </Section>

            <Section className="mt-6">
              <table
                role="presentation"
                width="100%"
                cellPadding="0"
                cellSpacing="0"
                border={0}
              >
                <tr>
                  <td width="32" valign="top" style={{ paddingTop: 2 }}>
                    <Text className="m-0 font-mono text-[14px] font-bold text-emerald">
                      03
                    </Text>
                  </td>
                  <td style={{ paddingLeft: 14 }}>
                    <Text className="m-0 text-[15px] font-semibold text-bright">
                      Break things
                    </Text>
                    <Text className="m-0 mt-1 text-[13px] leading-[1.65] text-muted">
                      Seriously. Find the edges. Hit reply on this email and
                      tell us what's off.
                    </Text>
                  </td>
                </tr>
              </table>
            </Section>

            {/* What's next */}
            <Section className="mt-14 rounded-xl bg-card p-7">
              <Text className="m-0 font-mono text-[10px] uppercase tracking-[0.25em] text-dim">
                What's next
              </Text>
              <Text className="m-0 mt-4 text-[14px] leading-[1.75] text-body">
                We'll take your feedback, ship fixes fast, and when it's
                ready&mdash;you'll get your full early access invite. You're one
                step away.
              </Text>
            </Section>

            {/* Divider */}
            <Hr className="mt-16 border-card" />

            {/* Footer */}
            <Text className="mt-10 text-center text-[11px] leading-relaxed text-subtle">
              You signed up at simse.dev.{" "}
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

EarlyPreviewEmail.PreviewProps = {
  previewUrl: "https://app.simse.dev/preview/abc123",
  unsubscribeUrl: "#",
} satisfies EarlyPreviewEmailProps;
