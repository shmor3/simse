import '@fontsource-variable/dm-sans';
import '@fontsource/space-mono';
import clsx from 'clsx';
import { Links, Meta, Outlet, Scripts, ScrollRestoration } from 'react-router';
import DotGrid from './components/DotGrid';
import './app.css';

export function Layout({ children }: { children: React.ReactNode }) {
	return (
		<html lang="en">
			<head>
				<meta charSet="utf-8" />
				<meta name="viewport" content="width=device-width, initial-scale=1" />
				<meta
					name="description"
					content="simse is a modular AI assistant that evolves with you. Connect any ACP or MCP backend. Context carries over. Preferences stick."
				/>
				<meta name="theme-color" content="#34d399" />
				<link rel="canonical" href="https://simse.dev/" />
				<link
					rel="icon"
					type="image/svg+xml"
					href="data:image/svg+xml,%3Csvg viewBox='0 0 100 100' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cdefs%3E%3CclipPath id='h'%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5'/%3E%3C/clipPath%3E%3C/defs%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5' fill='none' stroke='white' stroke-width='5'/%3E%3Cg clip-path='url(%23h)'%3E%3Cpath d='M44,-10 C90,15 94,35 50,50 C6,65 10,85 56,110' stroke='white' stroke-width='8' stroke-linecap='round' fill='none'/%3E%3C/g%3E%3C/svg%3E"
				/>
				<link rel="manifest" href="/site.webmanifest" />

				{/* Open Graph */}
				<meta property="og:type" content="website" />
				<meta property="og:url" content="https://simse.dev/" />
				<meta
					property="og:title"
					content="simse — The assistant that evolves with you"
				/>
				<meta
					property="og:description"
					content="Connect any ACP or MCP backend. Context carries over. Preferences stick. An assistant that gets better the more you use it."
				/>
				<meta property="og:image" content="https://simse.dev/og-image.png" />
				<meta property="og:image:width" content="1200" />
				<meta property="og:image:height" content="630" />

				{/* Twitter Card */}
				<meta name="twitter:card" content="summary_large_image" />
				<meta
					name="twitter:title"
					content="simse — The assistant that evolves with you"
				/>
				<meta
					name="twitter:description"
					content="Connect any ACP or MCP backend. Context carries over. Preferences stick. An assistant that gets better the more you use it."
				/>
				<meta name="twitter:image" content="https://simse.dev/og-image.png" />

				<title>simse — The assistant that evolves with you</title>
				<Meta />
				<Links />
			</head>
			<body>
				{children}
				<ScrollRestoration />
				<Scripts />
			</body>
		</html>
	);
}

export default function App() {
	return (
		<div
			className={clsx('flex h-screen flex-col overflow-hidden bg-[#0a0a0b]')}
		>
			<DotGrid />
			<Outlet />
		</div>
	);
}
