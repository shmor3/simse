import { Img } from '@react-email/components';

const LOGO_URL = 'https://simse.dev/logo-email.svg';

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
					<Img src={LOGO_URL} width="18" height="18" alt="simse" />
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
