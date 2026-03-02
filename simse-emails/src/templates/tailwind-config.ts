import type { TailwindConfig } from '@react-email/components';
import { pixelBasedPreset } from '@react-email/components';

export const emailTailwindConfig: TailwindConfig = {
	presets: [pixelBasedPreset],
	theme: {
		extend: {
			colors: {
				surface: '#0a0a0b',
				card: '#18181b',
				border: '#27272a',
				emerald: '#34d399',
				muted: '#71717a',
				subtle: '#3f3f46',
				dim: '#52525b',
				body: '#a1a1aa',
				light: '#d4d4d8',
				bright: '#e4e4e7',
			},
			fontFamily: {
				mono: ['Courier New', 'Courier', 'monospace'],
			},
		},
	},
};
