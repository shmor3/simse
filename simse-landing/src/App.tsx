import DotGrid from './components/DotGrid';
import Footer from './components/Footer';
import Hero from './components/Hero';

export default function App() {
	return (
		<div className="flex h-screen flex-col overflow-hidden bg-[#0a0a0b]">
			<DotGrid />
			<Hero />
			<Footer />
		</div>
	);
}
