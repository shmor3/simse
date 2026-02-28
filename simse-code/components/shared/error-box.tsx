import { Box, Text } from 'ink';

interface ErrorBoxProps {
	readonly message: string;
}

export function ErrorBox({ message }: ErrorBoxProps) {
	return (
		<Box paddingLeft={2}>
			<Text color="red">
				{'\u25cf'} {message}
			</Text>
		</Box>
	);
}
