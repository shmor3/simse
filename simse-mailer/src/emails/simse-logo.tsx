import { Img } from '@react-email/components';

const LOGO_SVG = `data:image/svg+xml,%3Csvg viewBox='0 0 100 100' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cdefs%3E%3CclipPath id='h'%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5'/%3E%3C/clipPath%3E%3C/defs%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5' fill='none' stroke='%23a1a1aa' stroke-width='3'/%3E%3Cg clip-path='url(%23h)'%3E%3Cpath d='M44,-10 C90,15 94,35 50,50 C6,65 10,85 56,110' stroke='%23a1a1aa' stroke-width='5.5' stroke-linecap='round' fill='none'/%3E%3Cpath d='M34,-10 C80,15 84,35 40,50 C-4,65 0,85 46,110' stroke='%23a1a1aa' stroke-width='3' stroke-linecap='round' fill='none' opacity='0.25'/%3E%3C/g%3E%3C/svg%3E`;

export default function SimseEmailLogo() {
	return (
		<table
			role="presentation"
			cellPadding="0"
			cellSpacing="0"
			border={0}
			style={{ margin: '0 auto' }}
		>
			<tr>
				<td style={{ verticalAlign: 'middle', paddingRight: '8px' }}>
					<Img src={LOGO_SVG} width="18" height="18" alt="" />
				</td>
				<td style={{ verticalAlign: 'middle' }}>
					<span
						style={{
							fontFamily: 'monospace',
							fontSize: '11px',
							fontWeight: 700,
							textTransform: 'uppercase' as const,
							letterSpacing: '0.35em',
							color: '#71717a',
						}}
					>
						SIMSE
					</span>
				</td>
			</tr>
		</table>
	);
}
