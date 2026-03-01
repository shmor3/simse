import type { Library } from './library.js';
import type { Lookup, Shelf, Volume } from './types.js';

export function createShelf(name: string, library: Library): Shelf {
	const add = async (
		text: string,
		metadata: Record<string, string> = {},
	): Promise<string> => {
		return library.add(text, { ...metadata, shelf: name });
	};

	const search = async (
		query: string,
		maxResults?: number,
		threshold?: number,
	): Promise<Lookup[]> => {
		const results = await library.search(query, maxResults, threshold);
		return results.filter((r) => r.volume.metadata.shelf === name);
	};

	const searchGlobal = async (
		query: string,
		maxResults?: number,
		threshold?: number,
	): Promise<Lookup[]> => {
		return library.search(query, maxResults, threshold);
	};

	const volumes = async (): Promise<Volume[]> => {
		return (await library.getAll()).filter((v) => v.metadata.shelf === name);
	};

	return Object.freeze({ name, add, search, searchGlobal, volumes });
}
