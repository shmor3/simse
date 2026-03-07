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
