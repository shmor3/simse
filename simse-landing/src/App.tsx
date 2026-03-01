import clsx from 'clsx';
import { Outlet } from 'react-router';
import DotGrid from './components/DotGrid';

export default function App() {
	return (
		<div
			className={clsx('flex h-screen flex-col overflow-hidden bg-[#0a0a0b]')}
		>
			<DotGrid />
			<Outlet />
		</div>
	);
}
