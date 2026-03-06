import ChatInterface from '~/components/chat/ChatInterface';

export default function Chat() {
	return (
		<ChatInterface
			messages={[]}
			toolCalls={[]}
			onSend={(msg) => console.log('send:', msg)}
		/>
	);
}
