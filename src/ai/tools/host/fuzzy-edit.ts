// ---------------------------------------------------------------------------
// Fuzzy Edit — 5-strategy matching for file edits
//
// Strategies tried in order:
// 1. Exact match
// 2. Line-trimmed (trim each line, compare)
// 3. Whitespace-normalized (collapse internal whitespace)
// 4. Indentation-flexible (strip common indent, re-indent replacement)
// 5. Block-anchor + Levenshtein (match first/last line, 30% tolerance)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface FuzzyMatchResult {
	readonly replaced: string;
	readonly strategy: string;
}

// ---------------------------------------------------------------------------
// Levenshtein distance
// ---------------------------------------------------------------------------

function levenshtein(a: string, b: string): number {
	const m = a.length;
	const n = b.length;

	if (m === 0) return n;
	if (n === 0) return m;

	// Use two rows instead of full matrix
	let prev = new Uint32Array(n + 1);
	let curr = new Uint32Array(n + 1);

	for (let j = 0; j <= n; j++) prev[j] = j;

	for (let i = 1; i <= m; i++) {
		curr[0] = i;
		for (let j = 1; j <= n; j++) {
			const cost = a[i - 1] === b[j - 1] ? 0 : 1;
			curr[j] = Math.min(
				prev[j] + 1, // deletion
				curr[j - 1] + 1, // insertion
				prev[j - 1] + cost, // substitution
			);
		}
		[prev, curr] = [curr, prev];
	}

	return prev[n];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function splitLines(text: string): string[] {
	return text.split('\n');
}

function getCommonIndent(lines: readonly string[]): string {
	const nonEmpty = lines.filter((l) => l.trim().length > 0);
	if (nonEmpty.length === 0) return '';

	let indent: string | undefined;
	for (const line of nonEmpty) {
		const match = line.match(/^(\s*)/);
		const lineIndent = match ? match[1] : '';
		if (indent === undefined || lineIndent.length < indent.length) {
			indent = lineIndent;
		}
	}
	return indent ?? '';
}

function stripCommonIndent(text: string): { stripped: string; indent: string } {
	const lines = splitLines(text);
	const indent = getCommonIndent(lines);
	if (indent.length === 0) return { stripped: text, indent: '' };

	const stripped = lines
		.map((line) => (line.startsWith(indent) ? line.slice(indent.length) : line))
		.join('\n');
	return { stripped, indent };
}

function reIndent(text: string, indent: string): string {
	if (indent.length === 0) return text;
	return splitLines(text)
		.map((line) => (line.trim().length > 0 ? indent + line : line))
		.join('\n');
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

function exactMatch(
	content: string,
	oldStr: string,
	newStr: string,
): FuzzyMatchResult | null {
	const index = content.indexOf(oldStr);
	if (index === -1) return null;

	// Ensure unique match
	if (content.indexOf(oldStr, index + 1) !== -1) return null;

	return {
		replaced:
			content.slice(0, index) + newStr + content.slice(index + oldStr.length),
		strategy: 'exact',
	};
}

function lineTrimmedMatch(
	content: string,
	oldStr: string,
	newStr: string,
): FuzzyMatchResult | null {
	const contentLines = splitLines(content);
	const oldLines = splitLines(oldStr);
	const trimmedOld = oldLines.map((l) => l.trim());

	if (oldLines.length === 0) return null;

	// Slide window over content lines
	let matchStart = -1;
	let matchCount = 0;

	for (let i = 0; i <= contentLines.length - oldLines.length; i++) {
		let matches = true;
		for (let j = 0; j < oldLines.length; j++) {
			if (contentLines[i + j].trim() !== trimmedOld[j]) {
				matches = false;
				break;
			}
		}
		if (matches) {
			matchCount++;
			if (matchCount > 1) return null; // Not unique
			matchStart = i;
		}
	}

	if (matchStart === -1) return null;

	const before = contentLines.slice(0, matchStart);
	const after = contentLines.slice(matchStart + oldLines.length);
	const result = [...before, newStr, ...after].join('\n');

	return { replaced: result, strategy: 'line-trimmed' };
}

function whitespaceNormalizedMatch(
	content: string,
	oldStr: string,
	newStr: string,
): FuzzyMatchResult | null {
	const contentLines = splitLines(content);
	const oldLines = splitLines(oldStr);
	const normalizedOld = oldLines.map((l) => l.trim().replace(/\s+/g, ' '));

	if (oldLines.length === 0) return null;

	let matchStart = -1;
	let matchCount = 0;

	for (let i = 0; i <= contentLines.length - oldLines.length; i++) {
		let matches = true;
		for (let j = 0; j < oldLines.length; j++) {
			const normalizedContent = contentLines[i + j].trim().replace(/\s+/g, ' ');
			if (normalizedContent !== normalizedOld[j]) {
				matches = false;
				break;
			}
		}
		if (matches) {
			matchCount++;
			if (matchCount > 1) return null;
			matchStart = i;
		}
	}

	if (matchStart === -1) return null;

	const before = contentLines.slice(0, matchStart);
	const after = contentLines.slice(matchStart + oldLines.length);
	const result = [...before, newStr, ...after].join('\n');

	return { replaced: result, strategy: 'whitespace-normalized' };
}

function indentationFlexibleMatch(
	content: string,
	oldStr: string,
	newStr: string,
): FuzzyMatchResult | null {
	const contentLines = splitLines(content);
	const { stripped: strippedOld } = stripCommonIndent(oldStr);
	const oldLines = splitLines(strippedOld);

	if (oldLines.length === 0) return null;

	let matchStart = -1;
	let matchIndent = '';
	let matchCount = 0;

	for (let i = 0; i <= contentLines.length - oldLines.length; i++) {
		// Get the indent of the first non-empty matched line
		const blockLines = contentLines.slice(i, i + oldLines.length);
		const { stripped: strippedBlock, indent } = stripCommonIndent(
			blockLines.join('\n'),
		);

		if (splitLines(strippedBlock).join('\n') === oldLines.join('\n')) {
			matchCount++;
			if (matchCount > 1) return null;
			matchStart = i;
			matchIndent = indent;
		}
	}

	if (matchStart === -1) return null;

	const before = contentLines.slice(0, matchStart);
	const after = contentLines.slice(matchStart + oldLines.length);
	const reIndented = reIndent(newStr, matchIndent);
	const result = [...before, reIndented, ...after].join('\n');

	return { replaced: result, strategy: 'indentation-flexible' };
}

function blockAnchorLevenshteinMatch(
	content: string,
	oldStr: string,
	newStr: string,
): FuzzyMatchResult | null {
	const contentLines = splitLines(content);
	const oldLines = splitLines(oldStr);

	if (oldLines.length < 2) return null;

	const firstOldTrimmed = oldLines[0].trim();
	const lastOldTrimmed = oldLines[oldLines.length - 1].trim();

	if (firstOldTrimmed.length === 0 || lastOldTrimmed.length === 0) return null;

	const tolerance = 0.3;
	let matchStart = -1;
	let matchEnd = -1;
	let matchCount = 0;

	for (let i = 0; i < contentLines.length; i++) {
		if (contentLines[i].trim() !== firstOldTrimmed) continue;

		// Found first-line anchor; search for last-line anchor
		const maxEnd = Math.min(
			i + oldLines.length + Math.ceil(oldLines.length * 0.5),
			contentLines.length,
		);

		for (let j = i + oldLines.length - 1; j < maxEnd; j++) {
			if (contentLines[j].trim() !== lastOldTrimmed) continue;

			// Check interior via Levenshtein
			const candidateBlock = contentLines.slice(i, j + 1).join('\n');
			const dist = levenshtein(oldStr, candidateBlock);
			const maxLen = Math.max(oldStr.length, candidateBlock.length);

			if (maxLen > 0 && dist / maxLen <= tolerance) {
				matchCount++;
				if (matchCount > 1) return null;
				matchStart = i;
				matchEnd = j;
			}
		}
	}

	if (matchStart === -1 || matchEnd === -1) return null;

	const before = contentLines.slice(0, matchStart);
	const after = contentLines.slice(matchEnd + 1);
	const result = [...before, newStr, ...after].join('\n');

	return { replaced: result, strategy: 'block-anchor-levenshtein' };
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export function fuzzyMatch(
	content: string,
	oldStr: string,
	newStr: string,
): FuzzyMatchResult | null {
	// Try strategies in order of strictness
	return (
		exactMatch(content, oldStr, newStr) ??
		lineTrimmedMatch(content, oldStr, newStr) ??
		whitespaceNormalizedMatch(content, oldStr, newStr) ??
		indentationFlexibleMatch(content, oldStr, newStr) ??
		blockAnchorLevenshteinMatch(content, oldStr, newStr)
	);
}
