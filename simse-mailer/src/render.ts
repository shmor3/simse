import { renderEmail } from './emails/index';

export async function renderTemplate(
	template: string,
	props: Record<string, string> = {},
): Promise<{ subject: string; html: string }> {
	return renderEmail(template, props);
}
