import ChatInterface from '~/components/chat/ChatInterface';
import type { Route } from './+types/dashboard.chat.$remoteId';

export default function RemoteChat({ params }: Route.ComponentProps) {
	return (
		<ChatInterface
			messages={[]}
			toolCalls={[]}
			onSend={(msg) => console.log('send to', params.remoteId, ':', msg)}
		/>
	);
}
