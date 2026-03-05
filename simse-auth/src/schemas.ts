import { z } from 'zod/v4';

export const registerSchema = z.object({
	name: z.string().min(2),
	email: z.email(),
	password: z.string().min(8),
});

export const loginSchema = z.object({
	email: z.email(),
	password: z.string(),
});

export const resetPasswordSchema = z.object({
	email: z.email(),
});

export const newPasswordSchema = z.object({
	token: z.string(),
	password: z.string().min(8),
});

export const twoFactorSchema = z.object({
	code: z.string().length(6),
	pendingToken: z.string(),
});

export const inviteSchema = z.object({
	email: z.email(),
	role: z.enum(['admin', 'member']),
});

export const updateNameSchema = z.object({
	name: z.string().min(2),
});

export const changePasswordSchema = z.object({
	currentPassword: z.string(),
	newPassword: z.string().min(8),
});

export const deleteAccountSchema = z.object({
	confirmEmail: z.string(),
});

export const createApiKeySchema = z.object({
	name: z.string().min(1).max(64),
});
