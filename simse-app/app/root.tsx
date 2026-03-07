import '@fontsource-variable/dm-sans';
import '@fontsource/space-mono';
import {
	isRouteErrorResponse,
	Links,
	Meta,
	Outlet,
	Scripts,
	ScrollRestoration,
} from 'react-router';
import type { Route } from './+types/root';
import './styles/app.css';

export function Layout({ children }: { children: React.ReactNode }) {
	return (
		<html lang="en">
			<head>
				<meta charSet="utf-8" />
				<meta name="viewport" content="width=device-width, initial-scale=1" />
				<title>simse</title>
				<meta name="theme-color" content="#0a0a0b" />
				<link
					rel="icon"
					type="image/svg+xml"
					href="data:image/svg+xml,%3Csvg viewBox='0 0 100 100' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cdefs%3E%3CclipPath id='h'%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5'/%3E%3C/clipPath%3E%3C/defs%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5' fill='none' stroke='white' stroke-width='5'/%3E%3Cg clip-path='url(%23h)'%3E%3Cpath d='M44,-10 C90,15 94,35 50,50 C6,65 10,85 56,110' stroke='white' stroke-width='8' stroke-linecap='round' fill='none'/%3E%3C/g%3E%3C/svg%3E"
				/>
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
	return <Outlet />;
}

export function ErrorBoundary({ error }: Route.ErrorBoundaryProps) {
	let heading = 'Something went wrong';
	let message = 'An unexpected error occurred.';

	if (isRouteErrorResponse(error)) {
		heading = `${error.status} ${error.statusText}`;
		message = error.data?.toString() ?? message;
	} else if (error instanceof Error) {
		message = error.message;
	}

	return (
		<div className="relative flex min-h-screen items-center justify-center bg-[#0a0a0b]">
			{/* Background glow */}
			<div
				className="pointer-events-none fixed inset-0"
				aria-hidden="true"
				style={{
					background:
						'radial-gradient(ellipse 50% 30% at 50% 50%, rgba(239, 68, 68, 0.04) 0%, transparent 70%)',
				}}
			/>
			<div className="relative z-10 max-w-md text-center animate-fade-in-up">
				<div className="flex items-center justify-center gap-2.5">
					<svg
						viewBox="0 0 100 100"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
						width={20}
						height={20}
						className="text-zinc-600"
					>
						<defs>
							<clipPath id="simse-hex-err">
								<polygon points="50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5" />
							</clipPath>
						</defs>
						<polygon
							points="50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5"
							fill="none"
							stroke="currentColor"
							strokeWidth={3.5}
						/>
						<g clipPath="url(#simse-hex-err)">
							<path
								d="M44,-10 C90,15 94,35 50,50 C6,65 10,85 56,110"
								stroke="currentColor"
								strokeWidth={6}
								strokeLinecap="round"
								fill="none"
							/>
							<path
								d="M34,-10 C80,15 84,35 40,50 C-4,65 0,85 46,110"
								stroke="currentColor"
								strokeWidth={3}
								strokeLinecap="round"
								fill="none"
								opacity={0.25}
							/>
						</g>
					</svg>
					<p className="font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-zinc-600">
						SIMSE
					</p>
				</div>
				{/* Error code */}
				{isRouteErrorResponse(error) && (
					<p className="mt-8 font-mono text-6xl font-bold text-zinc-800">
						{error.status}
					</p>
				)}
				<h1 className="mt-4 text-2xl font-bold tracking-tight text-white">
					{heading}
				</h1>
				<p className="mt-3 text-sm leading-relaxed text-zinc-500">{message}</p>
				<a
					href="/"
					className="mt-8 inline-flex items-center gap-2 rounded-lg bg-emerald-400 px-6 py-3 font-mono text-sm font-bold text-zinc-950 no-underline shadow-lg shadow-emerald-400/10 transition-all hover:bg-emerald-300 hover:shadow-emerald-400/20"
				>
					<svg
						className="h-4 w-4"
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
						strokeWidth={2}
					>
						<path
							strokeLinecap="round"
							strokeLinejoin="round"
							d="M10 19l-7-7m0 0l7-7m-7 7h18"
						/>
					</svg>
					Go home
				</a>
			</div>
		</div>
	);
}
