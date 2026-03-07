import { Outlet } from 'react-router';
import DotGrid from '../DotGrid';
import SimseLogo from '../ui/SimseLogo';

export default function AuthLayout() {
	return (
		<div className="relative flex min-h-screen items-center justify-center px-4">
			<DotGrid />
			<div className="relative z-10 w-full max-w-md animate-fade-in-up">
				{/* SIMSE header */}
				<div className="mb-10 flex items-center justify-center gap-2.5">
					<SimseLogo size={20} className="text-zinc-600" />
					<p className="font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-zinc-600">
						SIMSE
					</p>
				</div>

				{/* Card */}
				<div className="overflow-hidden rounded-xl border border-zinc-800 bg-zinc-900/80 shadow-2xl shadow-black/40 backdrop-blur-sm">
					{/* Animated gradient top bar */}
					<div className="h-1 gradient-border" />
					<div className="p-8">
						<Outlet />
					</div>
				</div>

				{/* Footer */}
				<p className="mt-8 text-center font-mono text-[11px] text-zinc-700">
					&copy; 2026 simse
				</p>
			</div>
		</div>
	);
}
