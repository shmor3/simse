// ---------------------------------------------------------------------------
// Shared mock factories for library tests
// ---------------------------------------------------------------------------

import type { Logger } from '../../src/ai/shared/logger.js';
import { createNoopLogger } from '../../src/ai/shared/logger.js';

// ---------------------------------------------------------------------------
// Silent Logger
// ---------------------------------------------------------------------------

export function createSilentLogger(): Logger {
	return createNoopLogger();
}
