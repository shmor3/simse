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
		<div className="flex min-h-screen items-center justify-center bg-[#0a0a0b]">
			<div className="max-w-md text-center">
				<p className="font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-zinc-600">
					SIMSE
				</p>
				<h1 className="mt-8 text-4xl font-bold tracking-tight text-white">
					{heading}
				</h1>
				<p className="mt-4 text-zinc-400">{message}</p>
				<a
					href="/"
					className="mt-8 inline-block rounded-lg bg-emerald-400 px-6 py-3 font-mono text-sm font-bold text-zinc-950 no-underline transition-colors hover:bg-emerald-300"
				>
					Go home
				</a>
			</div>
		</div>
	);
}
