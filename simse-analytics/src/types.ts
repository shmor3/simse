export interface Env {
	DB: D1Database;
	ANALYTICS: AnalyticsEngineDataset;
}

export interface DatapointMessage {
	type: 'datapoint';
	service: string;
	method: string;
	path: string;
	status: number;
	userId?: string;
	teamId?: string;
	country?: string;
	city?: string;
	continent?: string;
	userAgent?: string;
	referer?: string;
	contentType?: string;
	cfRay?: string;
	latencyMs: number;
	requestSize: number;
	responseSize: number;
	colo?: number;
}

export interface AuditMessage {
	type: 'audit';
	action: string;
	userId: string;
	timestamp: string;
	[key: string]: string;
}

export type AnalyticsMessage = DatapointMessage | AuditMessage;
