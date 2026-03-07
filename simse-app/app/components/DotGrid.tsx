import clsx from 'clsx';
import { useEffect, useRef } from 'react';

const GRID = 24;
const DOT_RADIUS = 1;
const BASE_OPACITY = 0.18;
const INFLUENCE_RADIUS = 200;
const MAX_RISE = 2;
const MAX_SCALE = 2;
const MAX_OPACITY_BOOST = 0.35;

export default function DotGrid() {
	const canvasRef = useRef<HTMLCanvasElement>(null);
	const mouse = useRef({ x: -1000, y: -1000 });
	const raf = useRef(0);

	useEffect(() => {
		const canvas = canvasRef.current;
		if (!canvas) return;
		const ctx = canvas.getContext('2d');
		if (!ctx) return;

		let w = 0;
		let h = 0;

		function resize() {
			const dpr = devicePixelRatio || 1;
			w = innerWidth;
			h = innerHeight;
			canvas!.width = w * dpr;
			canvas!.height = h * dpr;
			canvas!.style.width = `${w}px`;
			canvas!.style.height = `${h}px`;
			ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
		}

		function draw() {
			const mx = mouse.current.x;
			const my = mouse.current.y;

			ctx!.clearRect(0, 0, w, h);

			const cx = w / 2;
			const cy = h / 2;
			const maxDist = Math.hypot(cx, cy) * 0.9;
			const cols = Math.ceil(w / GRID) + 1;
			const rows = Math.ceil(h / GRID) + 1;

			for (let r = 0; r < rows; r++) {
				for (let c = 0; c < cols; c++) {
					const x = c * GRID;
					const y = r * GRID;

					const distCenter = Math.hypot(x - cx, y - cy);
					const fade = Math.max(0, 1 - distCenter / maxDist);
					if (fade <= 0) continue;

					const distMouse = Math.hypot(x - mx, y - my);
					const inf = Math.max(0, 1 - distMouse / INFLUENCE_RADIUS);
					const ease = inf * inf;

					const rise = ease * MAX_RISE;
					const scale = 1 + ease * (MAX_SCALE - 1);
					const alpha = (BASE_OPACITY + ease * MAX_OPACITY_BOOST) * fade;

					const cr = Math.round(255 - ease * 203);
					const cg = Math.round(255 - ease * 44);
					const cb = Math.round(255 - ease * 102);

					ctx!.beginPath();
					ctx!.arc(x, y - rise, DOT_RADIUS * scale, 0, Math.PI * 2);
					ctx!.fillStyle = `rgba(${cr},${cg},${cb},${alpha})`;
					ctx!.fill();
				}
			}

			raf.current = requestAnimationFrame(draw);
		}

		function onMove(e: MouseEvent) {
			mouse.current.x = e.clientX;
			mouse.current.y = e.clientY;
		}
		function onLeave() {
			mouse.current.x = -1000;
			mouse.current.y = -1000;
		}
		function onTouch(e: TouchEvent) {
			const t = e.touches[0];
			if (t) {
				mouse.current.x = t.clientX;
				mouse.current.y = t.clientY;
			}
		}
		function onTouchEnd() {
			mouse.current.x = -1000;
			mouse.current.y = -1000;
		}

		resize();
		addEventListener('resize', resize);
		addEventListener('mousemove', onMove);
		document.addEventListener('mouseleave', onLeave);
		addEventListener('touchstart', onTouch, { passive: true });
		addEventListener('touchmove', onTouch, { passive: true });
		addEventListener('touchend', onTouchEnd);
		raf.current = requestAnimationFrame(draw);

		return () => {
			removeEventListener('resize', resize);
			removeEventListener('mousemove', onMove);
			document.removeEventListener('mouseleave', onLeave);
			removeEventListener('touchstart', onTouch);
			removeEventListener('touchmove', onTouch);
			removeEventListener('touchend', onTouchEnd);
			cancelAnimationFrame(raf.current);
		};
	}, []);

	return (
		<>
			<canvas
				ref={canvasRef}
				className={clsx('pointer-events-none fixed inset-0 z-0')}
				aria-hidden="true"
			/>
			{/* Subtle emerald radial glow */}
			<div
				className="pointer-events-none fixed inset-0 z-0"
				aria-hidden="true"
				style={{
					background:
						'radial-gradient(ellipse 60% 40% at 50% 45%, rgba(52, 211, 153, 0.03) 0%, transparent 70%)',
				}}
			/>
		</>
	);
}
