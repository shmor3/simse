export default function Footer() {
	return (
		<footer className="px-6 py-5">
			<div className="flex justify-center">
				<span className="font-mono text-[11px] text-zinc-700">
					&copy; {new Date().getFullYear()} simse
				</span>
			</div>
		</footer>
	);
}
