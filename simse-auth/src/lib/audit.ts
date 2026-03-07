export type AuditAction =
	| 'password.changed'
	| 'password.reset'
	| 'account.deleted'
	| 'role.changed'
	| 'api_key.created'
	| 'api_key.deleted'
	| 'team.invited'
	| 'team.invite_deleted';

export function sendAuditEvent(
	queue: Queue,
	action: AuditAction,
	userId: string,
	meta?: Record<string, string>,
): void {
	// Fire-and-forget — never block the request
	queue
		.send({
			type: 'audit',
			action,
			userId,
			timestamp: new Date().toISOString(),
			...meta,
		})
		.catch(() => {});
}
