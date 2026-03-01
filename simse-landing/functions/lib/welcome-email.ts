const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="dark">
<meta name="supported-color-schemes" content="dark">
<title>Welcome to simse</title>
<!--[if mso]>
<style>table,td{font-family:Arial,sans-serif!important}</style>
<![endif]-->
</head>
<body style="margin:0;padding:0;background-color:#0a0a0b;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;-webkit-font-smoothing:antialiased;-moz-osx-font-smoothing:grayscale">

<!-- Outer wrapper -->
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="background-color:#0a0a0b">
<tr><td align="center" style="padding:48px 20px 40px">

<!-- Inner container -->
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="max-width:520px">

<!-- Header: mono label -->
<tr><td align="center" style="padding-bottom:48px">
<table role="presentation" cellpadding="0" cellspacing="0" border="0">
<tr><td style="padding:8px 16px;border:1px solid #27272a;border-radius:6px">
<span style="font-family:'Courier New',Courier,monospace;font-size:11px;letter-spacing:0.35em;color:#52525b;font-weight:700;text-transform:uppercase">SIMSE-CODE</span>
</td></tr>
</table>
</td></tr>

<!-- Emerald accent bar -->
<tr><td align="center" style="padding-bottom:40px">
<div style="width:48px;height:3px;background-color:#34d399;border-radius:2px"></div>
</td></tr>

<!-- Welcome heading -->
<tr><td align="center" style="padding-bottom:12px">
<h1 style="margin:0;font-size:32px;font-weight:700;line-height:1.15;color:#fafafa;letter-spacing:-0.025em">Welcome aboard.</h1>
</td></tr>

<!-- Subheading -->
<tr><td align="center" style="padding-bottom:40px">
<p style="margin:0;font-size:16px;line-height:1.5;color:#a1a1aa">You've secured your spot on the simse waitlist.</p>
</td></tr>

<!-- Card: what simse is -->
<tr><td style="padding-bottom:24px">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="background-color:#18181b;border:1px solid #27272a;border-radius:12px">
<tr><td style="padding:28px 28px 24px">

<!-- Card label -->
<table role="presentation" cellpadding="0" cellspacing="0" border="0" style="padding-bottom:16px">
<tr>
<td style="width:8px;height:8px;background-color:#34d399;border-radius:50%" width="8" height="8"></td>
<td style="padding-left:10px">
<span style="font-family:'Courier New',Courier,monospace;font-size:10px;letter-spacing:0.2em;color:#71717a;text-transform:uppercase">What we're building</span>
</td>
</tr>
</table>

<p style="margin:0;font-size:15px;line-height:1.75;color:#d4d4d8">
simse is an assistant that actually <span style="color:#34d399;font-weight:600">evolves</span> with you. Connect any AI backend via ACP, expose tools via MCP, and let context carry over across sessions. Preferences stick. The more you use it, the better it gets.
</p>

</td></tr>
</table>
</td></tr>

<!-- Card: what to expect -->
<tr><td style="padding-bottom:40px">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="background-color:#18181b;border:1px solid #27272a;border-radius:12px">
<tr><td style="padding:28px 28px 24px">

<!-- Card label -->
<table role="presentation" cellpadding="0" cellspacing="0" border="0" style="padding-bottom:16px">
<tr>
<td style="width:8px;height:8px;background-color:#34d399;border-radius:50%" width="8" height="8"></td>
<td style="padding-left:10px">
<span style="font-family:'Courier New',Courier,monospace;font-size:10px;letter-spacing:0.2em;color:#71717a;text-transform:uppercase">What happens next</span>
</td>
</tr>
</table>

<p style="margin:0 0 12px;font-size:15px;line-height:1.75;color:#d4d4d8">
As an early access member, you'll be among the first to try simse when we launch. Here's what to expect:
</p>

<!-- Bullet list -->
<table role="presentation" cellpadding="0" cellspacing="0" border="0" width="100%">
<tr>
<td width="20" valign="top" style="padding:4px 0;font-size:15px;color:#34d399;font-weight:700">&mdash;</td>
<td style="padding:4px 0;font-size:14px;line-height:1.65;color:#a1a1aa">Priority invite when we open early access</td>
</tr>
<tr>
<td width="20" valign="top" style="padding:4px 0;font-size:15px;color:#34d399;font-weight:700">&mdash;</td>
<td style="padding:4px 0;font-size:14px;line-height:1.65;color:#a1a1aa">Occasional updates on what we're shipping</td>
</tr>
<tr>
<td width="20" valign="top" style="padding:4px 0;font-size:15px;color:#34d399;font-weight:700">&mdash;</td>
<td style="padding:4px 0;font-size:14px;line-height:1.65;color:#a1a1aa">No spam &mdash; only meaningful milestones</td>
</tr>
</table>

</td></tr>
</table>
</td></tr>

<!-- Divider -->
<tr><td style="padding-bottom:32px">
<div style="height:1px;background-color:#27272a"></div>
</td></tr>

<!-- Unsubscribe -->
<tr><td align="center" style="padding-bottom:12px">
<p style="margin:0;font-size:11px;line-height:1.6;color:#3f3f46">
You received this because you signed up at simse.dev.
<a href="{{unsubscribe_url}}" style="color:#52525b;text-decoration:underline">Unsubscribe</a>
</p>
</td></tr>

<!-- Copyright -->
<tr><td align="center">
<span style="font-family:'Courier New',Courier,monospace;font-size:11px;color:#27272a">&copy; 2026 simse</span>
</td></tr>

</table>
<!-- /Inner container -->

</td></tr>
</table>
<!-- /Outer wrapper -->

</body>
</html>`;

export async function sendWelcomeEmail(
	email: string,
	apiKey: string,
	from: string,
	unsubscribeUrl: string,
): Promise<void> {
	const body = html.replace('{{unsubscribe_url}}', unsubscribeUrl);

	await fetch('https://api.resend.com/emails', {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${apiKey}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({
			from,
			to: email,
			subject: "You're on the simse waitlist",
			html: body,
			headers: {
				'List-Unsubscribe': `<${unsubscribeUrl}>`,
			},
		}),
	});
}
