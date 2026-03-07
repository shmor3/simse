import Footer from '~/components/Footer';
import Hero from '~/components/Hero';
import type { Route } from './+types/home';

export function meta(): Route.MetaDescriptors {
	return [{ title: 'simse — The assistant that evolves with you' }];
}

export default function Home() {
	return (
		<>
			<Hero />
			<Footer />
		</>
	);
}
