export async function sendEmail(
	queue: Queue,
	template: string,
	to: string,
	props: Record<string, string>,
): Promise<void> {
	await queue.send({ type: 'email', template, to, props });
}

export async function sendNotification(
	queue: Queue,
	userId: string,
	kind: string,
	title: string,
	body: string,
	link?: string,
): Promise<void> {
	await queue.send({ type: 'notification', userId, kind, title, body, link });
}
