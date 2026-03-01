import { useEffect, useLayoutEffect, useRef, useState } from 'react';

const words = [
	'coding',
	'design',
	'security',
	'research',
	'financial',
	'marketing',
	'operations',
];

const TYPE_SPEED = 100;
const DELETE_SPEED = 80;
const HOLD_DURATION = 2000;

export default function Typewriter() {
	const [wordIndex, setWordIndex] = useState(0);
	const [charIndex, setCharIndex] = useState(0);
	const [isDeleting, setIsDeleting] = useState(false);
	const [maxWidth, setMaxWidth] = useState(0);
	const containerRef = useRef<HTMLSpanElement>(null);

	// Measure widest word using a temporary off-screen element
	useLayoutEffect(() => {
		const el = document.createElement('span');
		el.style.cssText =
			'position:absolute;top:-9999px;left:-9999px;white-space:nowrap;visibility:hidden;pointer-events:none';
		// Copy font styles from the container once it's mounted
		if (containerRef.current) {
			const styles = getComputedStyle(containerRef.current);
			el.style.font = styles.font;
			el.style.letterSpacing = styles.letterSpacing;
		}
		document.body.appendChild(el);

		let widest = 0;
		for (const word of words) {
			el.textContent = word;
			widest = Math.max(widest, Math.ceil(el.getBoundingClientRect().width));
		}

		document.body.removeChild(el);
		// Add 3px for the inline cursor
		setMaxWidth(widest + 3);
	}, []);

	useEffect(() => {
		const word = words[wordIndex];

		if (!isDeleting && charIndex < word.length) {
			const timeout = setTimeout(() => setCharIndex((c) => c + 1), TYPE_SPEED);
			return () => clearTimeout(timeout);
		}

		if (!isDeleting && charIndex === word.length) {
			const timeout = setTimeout(() => setIsDeleting(true), HOLD_DURATION);
			return () => clearTimeout(timeout);
		}

		if (isDeleting && charIndex > 0) {
			const timeout = setTimeout(
				() => setCharIndex((c) => c - 1),
				DELETE_SPEED,
			);
			return () => clearTimeout(timeout);
		}

		if (isDeleting && charIndex === 0) {
			setIsDeleting(false);
			setWordIndex((i) => (i + 1) % words.length);
		}
	}, [wordIndex, charIndex, isDeleting]);

	const displayed = words[wordIndex].slice(0, charIndex);

	return (
		<span
			ref={containerRef}
			className="relative inline-block text-emerald-400"
			style={{ width: maxWidth > 0 ? `${maxWidth}px` : undefined }}
		>
			{/* Inline text with blinking border-right cursor */}
			<span className="typewriter-cursor">
				{'\u200B'}
				{displayed}
			</span>
			{/* Underline pinned to bottom */}
			<span className="absolute right-0 bottom-0 left-0 h-0.5 bg-emerald-400/30" />
		</span>
	);
}
