import clsx from 'clsx';

export default function Footer() {
	return (
		<footer className={clsx('px-6 py-5')}>
			<div className={clsx('flex justify-center')}>
				<span className={clsx('font-mono text-[11px] text-zinc-700')}>
					&copy; {new Date().getFullYear()} simse
				</span>
			</div>
		</footer>
	);
}
