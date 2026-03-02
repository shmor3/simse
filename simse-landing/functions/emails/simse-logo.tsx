import { Img } from '@react-email/components';

const LOGO_URL = 'https://simse.dev/logo-email.png';

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
				<td style={{ verticalAlign: 'middle' }}>
					<Img src={LOGO_URL} width="24" height="24" alt="simse" />
				</td>
			</tr>
		</table>
	);
}
