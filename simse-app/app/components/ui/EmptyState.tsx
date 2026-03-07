import { Link } from 'react-router';
import Button from './Button';
import Card from './Card';

type IllustrationType =
	| 'sessions'
	| 'library'
	| 'remotes'
	| 'usage'
	| 'notifications'
	| 'activity';

interface EmptyStateProps {
	type: IllustrationType;
	title: string;
	description: string;
	actionLabel?: string;
	actionTo?: string;
}

function Illustration({ type }: { type: IllustrationType }) {
	const shared = 'h-16 w-16';

	switch (type) {
		case 'sessions':
			return (
				<svg className={shared} viewBox="0 0 64 64" fill="none">
					<rect
						x="8"
						y="12"
						width="48"
						height="40"
						rx="6"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<circle cx="22" cy="32" r="2" fill="#52525b" />
					<circle cx="32" cy="32" r="2" fill="#52525b" />
					<circle cx="42" cy="32" r="2" fill="#52525b" />
					<path d="M8 22h48" stroke="#3f3f46" strokeWidth="1.5" />
					<circle cx="14" cy="17" r="1.5" fill="#34d399" opacity="0.4" />
					<circle cx="20" cy="17" r="1.5" fill="#52525b" />
					<circle cx="26" cy="17" r="1.5" fill="#52525b" />
				</svg>
			);
		case 'library':
			return (
				<svg className={shared} viewBox="0 0 64 64" fill="none">
					<rect
						x="14"
						y="10"
						width="12"
						height="44"
						rx="2"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<rect
						x="26"
						y="14"
						width="12"
						height="40"
						rx="2"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<rect
						x="38"
						y="8"
						width="12"
						height="46"
						rx="2"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<line
						x1="18"
						y1="18"
						x2="22"
						y2="18"
						stroke="#34d399"
						strokeWidth="1.5"
						opacity="0.5"
					/>
					<line
						x1="18"
						y1="22"
						x2="22"
						y2="22"
						stroke="#52525b"
						strokeWidth="1.5"
					/>
					<line
						x1="30"
						y1="22"
						x2="34"
						y2="22"
						stroke="#52525b"
						strokeWidth="1.5"
					/>
					<line
						x1="42"
						y1="16"
						x2="46"
						y2="16"
						stroke="#34d399"
						strokeWidth="1.5"
						opacity="0.5"
					/>
				</svg>
			);
		case 'remotes':
			return (
				<svg className={shared} viewBox="0 0 64 64" fill="none">
					<rect
						x="6"
						y="16"
						width="22"
						height="16"
						rx="3"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<rect
						x="36"
						y="16"
						width="22"
						height="16"
						rx="3"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<rect
						x="6"
						y="36"
						width="22"
						height="16"
						rx="3"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<path
						d="M28 24h8"
						stroke="#52525b"
						strokeWidth="1.5"
						strokeDasharray="2 2"
					/>
					<path
						d="M17 32v4"
						stroke="#52525b"
						strokeWidth="1.5"
						strokeDasharray="2 2"
					/>
					<circle cx="12" cy="22" r="1.5" fill="#34d399" opacity="0.5" />
					<circle cx="12" cy="42" r="1.5" fill="#52525b" />
					<circle cx="42" cy="22" r="1.5" fill="#52525b" />
				</svg>
			);
		case 'usage':
			return (
				<svg className={shared} viewBox="0 0 64 64" fill="none">
					<rect
						x="10"
						y="38"
						width="8"
						height="16"
						rx="2"
						fill="#3f3f46"
						opacity="0.3"
					/>
					<rect
						x="22"
						y="30"
						width="8"
						height="24"
						rx="2"
						fill="#3f3f46"
						opacity="0.3"
					/>
					<rect
						x="34"
						y="22"
						width="8"
						height="32"
						rx="2"
						fill="#3f3f46"
						opacity="0.3"
					/>
					<rect
						x="46"
						y="14"
						width="8"
						height="40"
						rx="2"
						fill="#34d399"
						opacity="0.15"
					/>
					<rect
						x="46"
						y="34"
						width="8"
						height="20"
						rx="2"
						fill="#34d399"
						opacity="0.3"
					/>
				</svg>
			);
		case 'notifications':
			return (
				<svg className={shared} viewBox="0 0 64 64" fill="none">
					<path
						d="M32 8a16 16 0 0116 16v10l4 6H12l4-6V24A16 16 0 0132 8z"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<path d="M26 44a6 6 0 0012 0" stroke="#3f3f46" strokeWidth="1.5" />
					<circle cx="32" cy="18" r="2" fill="#34d399" opacity="0.4" />
				</svg>
			);
		case 'activity':
			return (
				<svg className={shared} viewBox="0 0 64 64" fill="none">
					<circle cx="16" cy="16" r="4" stroke="#3f3f46" strokeWidth="1.5" />
					<line
						x1="16"
						y1="20"
						x2="16"
						y2="30"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<circle cx="16" cy="34" r="4" stroke="#3f3f46" strokeWidth="1.5" />
					<line
						x1="16"
						y1="38"
						x2="16"
						y2="48"
						stroke="#3f3f46"
						strokeWidth="1.5"
					/>
					<circle
						cx="16"
						cy="52"
						r="4"
						stroke="#34d399"
						strokeWidth="1.5"
						opacity="0.4"
					/>
					<line
						x1="26"
						y1="16"
						x2="50"
						y2="16"
						stroke="#52525b"
						strokeWidth="1.5"
					/>
					<line
						x1="26"
						y1="34"
						x2="44"
						y2="34"
						stroke="#52525b"
						strokeWidth="1.5"
					/>
					<line
						x1="26"
						y1="52"
						x2="38"
						y2="52"
						stroke="#52525b"
						strokeWidth="1.5"
						opacity="0.4"
					/>
				</svg>
			);
	}
}

export default function EmptyState({
	type,
	title,
	description,
	actionLabel,
	actionTo,
}: EmptyStateProps) {
	return (
		<Card className="p-10 text-center">
			<div className="mx-auto flex items-center justify-center">
				<Illustration type={type} />
			</div>
			<p className="mt-5 text-sm font-medium text-zinc-400">{title}</p>
			<p className="mt-1.5 text-[13px] text-zinc-600">{description}</p>
			{actionLabel && actionTo && (
				<div className="mt-5">
					<Link to={actionTo}>
						<Button variant="secondary">{actionLabel}</Button>
					</Link>
				</div>
			)}
		</Card>
	);
}
