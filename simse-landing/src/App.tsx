import Hero from './components/Hero';
import Features from './components/Features';
import Footer from './components/Footer';

export default function App() {
	return (
		<div className="relative min-h-screen bg-zinc-950 text-zinc-50">
			{/* Dot grid background */}
			<div className="dot-grid pointer-events-none fixed inset-0" />

			<Hero />
			<Features />
			<Footer />
		</div>
	);
}
