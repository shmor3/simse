import { z } from 'zod/v4';

const passwordSchema = z
	.string()
	.min(8)
	.max(128)
	.refine(
		(p) => /[a-zA-Z]/.test(p),
		'Password must contain at least one letter',
	)
	.refine((p) => /[0-9]/.test(p), 'Password must contain at least one digit');

export const registerSchema = z.object({
	name: z.string().min(2),
	email: z.email(),
	password: passwordSchema,
});

export const loginSchema = z.object({
	email: z.email(),
	password: z.string().max(128),
});

export const resetPasswordSchema = z.object({
	email: z.email(),
});

export const newPasswordSchema = z.object({
	token: z.string(),
	password: passwordSchema,
});

export const twoFactorSchema = z.object({
	code: z.string().length(6),
	pendingToken: z.string(),
});

export const verifyEmailSchema = z.object({
	code: z.string().length(6),
});

export const inviteSchema = z.object({
	email: z.email(),
	role: z.enum(['admin', 'member']),
});

export const updateNameSchema = z.object({
	name: z.string().min(2),
});

export const changePasswordSchema = z.object({
	currentPassword: z.string().max(128),
	newPassword: passwordSchema,
});

export const deleteAccountSchema = z.object({
	confirmEmail: z.string(),
});

export const createApiKeySchema = z.object({
	name: z.string().min(1).max(64),
});
