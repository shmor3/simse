import clsx from 'clsx';
import { useEffect, useRef } from 'react';
import Button from './Button';

interface ModalProps {
	open: boolean;
	onClose: () => void;
	title: string;
	description?: string;
	confirmLabel?: string;
	confirmVariant?: 'primary' | 'danger';
	onConfirm?: () => void;
	loading?: boolean;
	children?: React.ReactNode;
}

export default function Modal({
	open,
	onClose,
	title,
	description,
	confirmLabel = 'Confirm',
	confirmVariant = 'primary',
	onConfirm,
	loading,
	children,
}: ModalProps) {
	const overlayRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (!open) return;
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape') onClose();
		}
		document.addEventListener('keydown', onKey);
		return () => document.removeEventListener('keydown', onKey);
	}, [open, onClose]);

	if (!open) return null;

	return (
		<div
			ref={overlayRef}
			role="presentation"
			className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in"
			onClick={(e) => {
				if (e.target === overlayRef.current) onClose();
			}}
			onKeyDown={(e) => {
				if (e.key === 'Escape') onClose();
			}}
		>
			<div
				className={clsx(
					'w-full max-w-md rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-2xl animate-fade-in-up',
				)}
			>
				<h2 className="text-lg font-bold text-white">{title}</h2>
				{description && (
					<p className="mt-2 text-sm text-zinc-400">{description}</p>
				)}
				{children && <div className="mt-4">{children}</div>}
				<div className="mt-6 flex justify-end gap-3">
					<Button variant="ghost" onClick={onClose} disabled={loading}>
						Cancel
					</Button>
					{onConfirm && (
						<Button
							variant={confirmVariant}
							onClick={onConfirm}
							loading={loading}
						>
							{confirmLabel}
						</Button>
					)}
				</div>
			</div>
		</div>
	);
}
