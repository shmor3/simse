import 'react-router';

declare module 'react-router' {
	interface AppLoadContext {
		cloudflare: {
			env: {
				DB: D1Database;
				COMMS_QUEUE: Queue;
				ANALYTICS: AnalyticsEngineDataset;
			};
			ctx: ExecutionContext;
		};
	}
}
