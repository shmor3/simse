import { z } from 'zod/v4';

export const waitlistSchema = z.object({
	email: z
		.email('Please enter a valid email address'),
});
