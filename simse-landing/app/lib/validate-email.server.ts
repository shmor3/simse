// Common disposable/temporary email domains
const DISPOSABLE_DOMAINS = new Set([
	'mailinator.com',
	'guerrillamail.com',
	'guerrillamail.de',
	'grr.la',
	'guerrillamailblock.com',
	'tempmail.com',
	'temp-mail.org',
	'throwaway.email',
	'yopmail.com',
	'yopmail.fr',
	'sharklasers.com',
	'guerrillamail.info',
	'guerrillamail.net',
	'dispostable.com',
	'trashmail.com',
	'trashmail.me',
	'trashmail.net',
	'mailnesia.com',
	'maildrop.cc',
	'discard.email',
	'mailcatch.com',
	'tempail.com',
	'fakeinbox.com',
	'mailnull.com',
	'jetable.org',
	'10minutemail.com',
	'10minute.email',
	'minutemail.com',
	'tempr.email',
	'tempinbox.com',
	'burnermail.io',
	'getairmail.com',
	'mailexpire.com',
	'throwam.com',
	'getnada.com',
	'emailondeck.com',
	'33mail.com',
	'spamgourmet.com',
	'mytemp.email',
	'mohmal.com',
	'harakirimail.com',
	'crazymailing.com',
	'tmail.com',
	'mailsac.com',
	'inboxkitten.com',
	'receiveee.com',
]);

function isDisposable(domain: string): boolean {
	return DISPOSABLE_DOMAINS.has(domain);
}

interface DnsAnswer {
	type: number;
	data: string;
}

interface DnsResponse {
	Status: number;
	Answer?: DnsAnswer[];
}

async function hasMxRecords(domain: string): Promise<boolean> {
	try {
		const res = await fetch(
			`https://cloudflare-dns.com/dns-query?name=${encodeURIComponent(domain)}&type=MX`,
			{
				headers: { Accept: 'application/dns-json' },
			},
		);

		if (!res.ok) return true; // fail open if DNS lookup errors

		const data: DnsResponse = await res.json();

		// Status 0 = NOERROR, check for MX records (type 15)
		if (data.Status !== 0) return false;
		return !!data.Answer?.some((a) => a.type === 15);
	} catch {
		return true; // fail open on network errors
	}
}

export type ValidationResult =
	| { valid: true }
	| { valid: false; reason: string };

export async function validateEmail(email: string): Promise<ValidationResult> {
	const domain = email.split('@')[1];
	if (!domain) {
		return { valid: false, reason: 'Invalid email format' };
	}

	if (isDisposable(domain)) {
		return {
			valid: false,
			reason: 'Disposable email addresses are not allowed',
		};
	}

	if (!(await hasMxRecords(domain))) {
		return {
			valid: false,
			reason: 'This email domain does not appear to accept mail',
		};
	}

	return { valid: true };
}
