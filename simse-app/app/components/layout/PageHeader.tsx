interface PageHeaderProps {
	title: string;
	description?: string;
	action?: React.ReactNode;
}

export default function PageHeader({
	title,
	description,
	action,
}: PageHeaderProps) {
	return (
		<div className="flex items-start justify-between">
			<div>
				<h1 className="text-2xl font-bold tracking-tight text-white">
					{title}
				</h1>
				{description && (
					<p className="mt-1 text-sm text-zinc-500">{description}</p>
				)}
			</div>
			{action && <div>{action}</div>}
		</div>
	);
}
