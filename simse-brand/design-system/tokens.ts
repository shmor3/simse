export const colors = {
	emerald: '#34d399',
	dark: '#0a0a0b',
	white: '#ffffff',
	success: '#34d399',
	error: '#ff6568',
	warning: '#fbbf24',
	info: '#60a5fa',
	zinc: {
		50: '#fafafa',
		100: '#f4f4f5',
		200: '#e4e4e7',
		300: '#d4d4d8',
		400: '#a1a1aa',
		500: '#71717a',
		600: '#52525b',
		700: '#3f3f46',
		800: '#27272a',
		900: '#18181b',
		950: '#09090b',
	},
} as const;

export const fonts = {
	sans: "'DM Sans Variable', system-ui, sans-serif",
	mono: "'Space Mono', ui-monospace, monospace",
} as const;

export const radius = { sm: '6px', md: '8px', lg: '12px', full: '9999px' } as const;
export const layout = { railWidth: 56, navWidth: 220, headerHeight: 56 } as const;
export const duration = { fast: 200, normal: 500, slow: 600 } as const;
