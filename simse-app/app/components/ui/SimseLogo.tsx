import clsx from 'clsx';

interface SimseLogoProps {
	size?: number;
	className?: string;
}

export default function SimseLogo({ size = 22, className }: SimseLogoProps) {
	return (
		<svg
			viewBox="0 0 100 100"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
			width={size}
			height={size}
			className={clsx(className)}
		>
			<defs>
				<clipPath id="simse-hex">
					<polygon points="50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5" />
				</clipPath>
			</defs>
			<polygon
				points="50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5"
				fill="none"
				stroke="currentColor"
				strokeWidth={size <= 24 ? 3.5 : 2}
			/>
			<g clipPath="url(#simse-hex)">
				<path
					d="M44,-10 C90,15 94,35 50,50 C6,65 10,85 56,110"
					stroke="currentColor"
					strokeWidth={size <= 24 ? 6 : 4}
					strokeLinecap="round"
					fill="none"
				/>
				<path
					d="M34,-10 C80,15 84,35 40,50 C-4,65 0,85 46,110"
					stroke="currentColor"
					strokeWidth={size <= 24 ? 3 : 2.5}
					strokeLinecap="round"
					fill="none"
					opacity={0.25}
				/>
				<path
					d="M54,-10 C100,15 104,35 60,50 C16,65 20,85 66,110"
					stroke="currentColor"
					strokeWidth={size <= 24 ? 1.5 : 1.5}
					strokeLinecap="round"
					fill="none"
					opacity={0.1}
				/>
			</g>
		</svg>
	);
}
