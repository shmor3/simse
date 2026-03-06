export async function sendEmail(
	queue: Queue,
	template: string,
	to: string,
	props: Record<string, string>,
): Promise<void> {
	try {
		await queue.send({ type: 'email', template, to, props });
	} catch (err) {
		console.error('Failed to queue email', { template, to, error: err });
	}
}

export async function sendNotification(
	queue: Queue,
	userId: string,
	kind: string,
	title: string,
	body: string,
	link?: string,
): Promise<void> {
	try {
		await queue.send({ type: 'notification', userId, kind, title, body, link });
	} catch (err) {
		console.error('Failed to queue notification', { kind, userId, error: err });
	}
}
