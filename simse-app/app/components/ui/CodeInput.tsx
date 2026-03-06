import clsx from 'clsx';
import { useCallback, useRef, useState } from 'react';

interface CodeInputProps {
	length?: number;
	name?: string;
	error?: string;
	onComplete?: (code: string) => void;
}

export default function CodeInput({
	length = 6,
	name = 'code',
	error,
	onComplete,
}: CodeInputProps) {
	const [values, setValues] = useState<string[]>(Array(length).fill(''));
	const inputs = useRef<(HTMLInputElement | null)[]>([]);

	const focusAt = useCallback(
		(i: number) => {
			if (i >= 0 && i < length) inputs.current[i]?.focus();
		},
		[length],
	);

	const handleChange = useCallback(
		(i: number, val: string) => {
			const digit = val.replace(/\D/g, '').slice(-1);
			const next = [...values];
			next[i] = digit;
			setValues(next);

			if (digit && i < length - 1) focusAt(i + 1);

			const code = next.join('');
			if (code.length === length && !code.includes('')) {
				onComplete?.(code);
			}
		},
		[values, length, focusAt, onComplete],
	);

	const handleKeyDown = useCallback(
		(i: number, e: React.KeyboardEvent) => {
			if (e.key === 'Backspace' && !values[i] && i > 0) {
				focusAt(i - 1);
			}
			if (e.key === 'ArrowLeft') focusAt(i - 1);
			if (e.key === 'ArrowRight') focusAt(i + 1);
		},
		[values, focusAt],
	);

	const handlePaste = useCallback(
		(e: React.ClipboardEvent) => {
			e.preventDefault();
			const text = e.clipboardData.getData('text').replace(/\D/g, '');
			const next = [...values];
			for (let j = 0; j < Math.min(text.length, length); j++) {
				next[j] = text[j];
			}
			setValues(next);
			focusAt(Math.min(text.length, length) - 1);

			const code = next.join('');
			if (code.length === length) onComplete?.(code);
		},
		[values, length, focusAt, onComplete],
	);

	const code = values.join('');

	return (
		<div className="space-y-2">
			<div className="flex justify-center gap-2">
				{values.map((v, i) => (
					<input
						key={i}
						ref={(el) => {
							inputs.current[i] = el;
						}}
						type="text"
						inputMode="numeric"
						maxLength={1}
						value={v}
						onChange={(e) => handleChange(i, e.target.value)}
						onKeyDown={(e) => handleKeyDown(i, e)}
						onPaste={i === 0 ? handlePaste : undefined}
						className={clsx(
							'h-14 w-11 rounded-lg border bg-zinc-900 text-center font-mono text-xl font-bold text-white transition-colors focus:border-emerald-400/50 focus:outline-none focus:ring-1 focus:ring-emerald-400/25',
							error
								? 'border-red-500/50'
								: 'border-zinc-800 hover:border-zinc-700',
						)}
						aria-label={`Digit ${i + 1}`}
					/>
				))}
			</div>
			<input type="hidden" name={name} value={code} />
			{error && (
				<p className="text-center text-[13px] text-red-400/80">{error}</p>
			)}
		</div>
	);
}
