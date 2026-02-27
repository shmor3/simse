import { createContext, useContext, useMemo } from 'react';
import type { ReactNode } from 'react';
import { createColors, createMarkdownRenderer } from '../ui.js';
import type { MarkdownRenderer, TermColors } from '../ui.js';

interface ThemeContextValue {
	readonly colors: TermColors;
	readonly md: MarkdownRenderer;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme(): ThemeContextValue {
	const ctx = useContext(ThemeContext);
	if (!ctx) throw new Error('useTheme must be used within a ThemeProvider');
	return ctx;
}

interface ThemeProviderProps {
	readonly children: ReactNode;
	readonly forceColors?: boolean;
}

export function ThemeProvider({ children, forceColors }: ThemeProviderProps) {
	const value = useMemo(() => {
		const colors = createColors({ enabled: forceColors });
		const md = createMarkdownRenderer(colors);
		return { colors, md } as const;
	}, [forceColors]);

	return (
		<ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
	);
}
