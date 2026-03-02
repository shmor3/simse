export function generateId(): string {
	return crypto.randomUUID();
}

export async function queryOne<T>(
	db: D1Database,
	sql: string,
	...params: unknown[]
): Promise<T | null> {
	const stmt = db.prepare(sql);
	if (params.length > 0) stmt.bind(...params);
	return stmt.first<T>();
}

export async function queryAll<T>(
	db: D1Database,
	sql: string,
	...params: unknown[]
): Promise<T[]> {
	const stmt = db.prepare(sql);
	if (params.length > 0) stmt.bind(...params);
	const result = await stmt.all<T>();
	return result.results;
}

export async function execute(
	db: D1Database,
	sql: string,
	...params: unknown[]
): Promise<D1Result> {
	const stmt = db.prepare(sql);
	if (params.length > 0) stmt.bind(...params);
	return stmt.run();
}
