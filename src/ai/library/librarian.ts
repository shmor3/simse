// ---------------------------------------------------------------------------
// Librarian — LLM-driven memory extraction, summarization, classification,
// and reorganization.
// ---------------------------------------------------------------------------

import type {
	ClassificationResult,
	ExtractionMemory,
	ExtractionResult,
	Librarian,
	ReorganizationPlan,
	TextGenerationProvider,
	TurnContext,
	Volume,
} from './types.js';

export function createLibrarian(textGenerator: TextGenerationProvider): Librarian {
	const extract = async (turn: TurnContext): Promise<ExtractionResult> => {
		const prompt = `Analyze this conversation turn and extract important information worth remembering.

User: ${turn.userInput}
Assistant: ${turn.response}

Return a JSON object with this structure:
{
  "memories": [
    {
      "text": "concise fact or decision",
      "topic": "hierarchical/topic/path",
      "tags": ["tag1", "tag2"],
      "entryType": "fact" | "decision" | "observation"
    }
  ]
}

Rules:
- Only extract genuinely important facts, decisions, or observations
- Skip trivial conversational content
- Use hierarchical topic paths separated by /
- Return {"memories": []} if nothing worth remembering

Respond with ONLY valid JSON, no other text.`;

		try {
			const response = await textGenerator.generate(prompt);
			const parsed = JSON.parse(response);
			if (!parsed.memories || !Array.isArray(parsed.memories)) {
				return { memories: [] };
			}
			// Validate each memory entry
			const validMemories: ExtractionMemory[] = parsed.memories
				.filter(
					(m: Record<string, unknown>) =>
						typeof m.text === 'string' &&
						typeof m.topic === 'string' &&
						Array.isArray(m.tags) &&
						['fact', 'decision', 'observation'].includes(
							m.entryType as string,
						),
				)
				.map((m: Record<string, unknown>) => ({
					text: m.text as string,
					topic: m.topic as string,
					tags: (m.tags as string[]).map(String),
					entryType: m.entryType as 'fact' | 'decision' | 'observation',
				}));
			return { memories: validMemories };
		} catch {
			return { memories: [] };
		}
	};

	const summarize = async (
		volumes: readonly Volume[],
		topic: string,
	): Promise<{ text: string; sourceIds: readonly string[] }> => {
		const combinedText = volumes
			.map((v, i) => `--- Volume ${i + 1} ---\n${v.text}`)
			.join('\n\n');

		const prompt = `Summarize the following volumes from topic "${topic}" into a single concise summary that preserves all key information:\n\n${combinedText}`;

		const text = await textGenerator.generate(prompt);
		return {
			text,
			sourceIds: volumes.map((v) => v.id),
		};
	};

	const classifyTopic = async (
		text: string,
		existingTopics: readonly string[],
	): Promise<ClassificationResult> => {
		const prompt = `Classify the following text into the most appropriate topic.

Text: ${text}

Existing topics:
${existingTopics.map((t) => `- ${t}`).join('\n')}

Return a JSON object:
{"topic": "best/topic/path", "confidence": 0.0-1.0}

You may suggest a new subtopic if none of the existing ones fit well.
Respond with ONLY valid JSON.`;

		try {
			const response = await textGenerator.generate(prompt);
			const parsed = JSON.parse(response);
			return {
				topic: String(parsed.topic ?? 'uncategorized'),
				confidence: Number(parsed.confidence ?? 0),
			};
		} catch {
			return { topic: 'uncategorized', confidence: 0 };
		}
	};

	const reorganize = async (
		topic: string,
		volumes: readonly Volume[],
	): Promise<ReorganizationPlan> => {
		const volumeList = volumes
			.map((v) => `- [${v.id}] ${v.text}`)
			.join('\n');

		const prompt = `Review the following volumes in topic "${topic}" and suggest reorganization.

Volumes:
${volumeList}

Return a JSON object:
{
  "moves": [{"volumeId": "id", "newTopic": "new/topic/path"}],
  "newSubtopics": ["new/subtopic"],
  "merges": [{"source": "topic/a", "target": "topic/b"}]
}

Respond with ONLY valid JSON.`;

		try {
			const response = await textGenerator.generate(prompt);
			const parsed = JSON.parse(response);
			return {
				moves: Array.isArray(parsed.moves) ? parsed.moves : [],
				newSubtopics: Array.isArray(parsed.newSubtopics)
					? parsed.newSubtopics
					: [],
				merges: Array.isArray(parsed.merges) ? parsed.merges : [],
			};
		} catch {
			return { moves: [], newSubtopics: [], merges: [] };
		}
	};

	return Object.freeze({ extract, summarize, classifyTopic, reorganize });
}
