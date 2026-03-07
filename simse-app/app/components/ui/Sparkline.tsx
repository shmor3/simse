interface SparklineProps {
	data: number[];
	width?: number;
	height?: number;
	className?: string;
}

export default function Sparkline({
	data,
	width = 80,
	height = 28,
	className,
}: SparklineProps) {
	if (data.length < 2) return null;

	const max = Math.max(...data, 1);
	const min = Math.min(...data, 0);
	const range = max - min || 1;
	const padY = 2;

	const points = data.map((v, i) => {
		const x = (i / (data.length - 1)) * width;
		const y = height - padY - ((v - min) / range) * (height - padY * 2);
		return { x, y };
	});

	const linePath = points
		.map((p, i) => `${i === 0 ? 'M' : 'L'}${p.x},${p.y}`)
		.join(' ');

	const areaPath = `${linePath} L${width},${height} L0,${height} Z`;

	return (
		<svg
			width={width}
			height={height}
			viewBox={`0 0 ${width} ${height}`}
			className={className}
			fill="none"
		>
			<defs>
				<linearGradient id="sparkline-fill" x1="0" y1="0" x2="0" y2="1">
					<stop offset="0%" stopColor="#34d399" stopOpacity="0.2" />
					<stop offset="100%" stopColor="#34d399" stopOpacity="0" />
				</linearGradient>
			</defs>
			<path d={areaPath} fill="url(#sparkline-fill)" />
			<path
				d={linePath}
				stroke="#34d399"
				strokeWidth={1.5}
				strokeLinecap="round"
				strokeLinejoin="round"
			/>
			{/* End dot */}
			<circle
				cx={points[points.length - 1].x}
				cy={points[points.length - 1].y}
				r={2}
				fill="#34d399"
			/>
		</svg>
	);
}
