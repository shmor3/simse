import { z } from 'zod';

export const loginSchema = z.object({
	email: z.email('Please enter a valid email'),
	password: z.string().min(1, 'Password is required'),
});

export const registerSchema = z.object({
	name: z.string().min(2, 'Name must be at least 2 characters'),
	email: z.email('Please enter a valid email'),
	password: z.string().min(8, 'Password must be at least 8 characters'),
});

export const resetPasswordSchema = z.object({
	email: z.email('Please enter a valid email'),
});

export const newPasswordSchema = z.object({
	token: z.string().min(1),
	password: z.string().min(8, 'Password must be at least 8 characters'),
});

export const twoFactorSchema = z.object({
	code: z.string().length(6, 'Code must be 6 digits'),
});

export const inviteSchema = z.object({
	email: z.email('Please enter a valid email'),
	role: z.enum(['admin', 'member']),
});
