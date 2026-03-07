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
	name: z.string().trim().min(2).max(100),
	email: z.email().max(254),
	password: passwordSchema,
});

export const loginSchema = z.object({
	email: z.email().max(254),
	password: z.string().max(128),
});

export const resetPasswordSchema = z.object({
	email: z.email().max(254),
});

export const newPasswordSchema = z.object({
	token: z.string().regex(/^\d{6}$/),
	password: passwordSchema,
});

export const twoFactorSchema = z.object({
	code: z.string().regex(/^\d{6}$/),
	pendingToken: z.string().max(64),
});

export const verifyEmailSchema = z.object({
	code: z.string().regex(/^\d{6}$/),
});

export const inviteSchema = z.object({
	email: z.email().max(254),
	role: z.enum(['admin', 'member']),
});

export const updateNameSchema = z.object({
	name: z.string().trim().min(2).max(100),
});

export const changePasswordSchema = z.object({
	currentPassword: z.string().max(128),
	newPassword: passwordSchema,
});

export const deleteAccountSchema = z.object({
	confirmEmail: z.email().max(254),
	password: z.string().max(128),
});

export const createApiKeySchema = z.object({
	name: z.string().trim().min(1).max(64),
});

export const refreshSchema = z.object({
	refreshToken: z.string().startsWith('rt_').max(128),
});

export const revokeSchema = z.object({
	refreshToken: z.string().startsWith('rt_').max(128),
});

export const updateRoleSchema = z.object({
	role: z.enum(['admin', 'member']),
});
