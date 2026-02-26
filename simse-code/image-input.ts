/**
 * SimSE Code — Image Input
 *
 * Detects image file paths in user input, reads and base64-encodes them,
 * and provides content blocks for ACP multimodal messages.
 * No external deps.
 */

import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, resolve } from 'node:path';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ImageAttachment {
	readonly path: string;
	readonly mimeType: string;
	readonly base64: string;
	readonly size: number;
}

export interface ImageDetectionResult {
	/** User input with image paths removed. */
	readonly cleanInput: string;
	/** Detected and encoded images. */
	readonly images: readonly ImageAttachment[];
}

export interface ImageInputOptions {
	/** Base directory for resolving relative paths. Default: process.cwd() */
	readonly baseDir?: string;
	/** Max image file size (bytes). Default: 10MB */
	readonly maxImageSize?: number;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const IMAGE_EXTENSIONS: Readonly<Record<string, string>> = {
	'.png': 'image/png',
	'.jpg': 'image/jpeg',
	'.jpeg': 'image/jpeg',
	'.gif': 'image/gif',
	'.webp': 'image/webp',
	'.bmp': 'image/bmp',
	'.svg': 'image/svg+xml',
};

const DEFAULT_MAX_SIZE = 10 * 1024 * 1024; // 10MB

// Pattern to match file paths that look like images
const IMAGE_PATH_PATTERN =
	/(?:^|\s)((?:\.{0,2}[\\/])?[\w./-]+\.(?:png|jpg|jpeg|gif|webp|bmp|svg))(?:\s|$)/gi;

// ---------------------------------------------------------------------------
// Detection and encoding
// ---------------------------------------------------------------------------

/**
 * Detect image file paths in user input, read and encode them.
 */
export function detectImages(
	input: string,
	options?: ImageInputOptions,
): ImageDetectionResult {
	const baseDir = options?.baseDir ?? process.cwd();
	const maxSize = options?.maxImageSize ?? DEFAULT_MAX_SIZE;
	const images: ImageAttachment[] = [];
	const seen = new Set<string>();

	let cleanInput = input;
	let match: RegExpExecArray | null = IMAGE_PATH_PATTERN.exec(input);

	while (match !== null) {
		const rawPath = match[1].trim();
		const fullPath = resolve(baseDir, rawPath);

		if (!seen.has(fullPath) && existsSync(fullPath)) {
			try {
				const stat = statSync(fullPath);
				if (stat.isFile() && stat.size <= maxSize) {
					const ext = extname(rawPath).toLowerCase();
					const mimeType = IMAGE_EXTENSIONS[ext];
					if (mimeType) {
						const buffer = readFileSync(fullPath);
						images.push(
							Object.freeze({
								path: rawPath,
								mimeType,
								base64: buffer.toString('base64'),
								size: stat.size,
							}),
						);
						seen.add(fullPath);
					}
				}
			} catch {
				// Skip unreadable files
			}
		}

		match = IMAGE_PATH_PATTERN.exec(input);
	}

	// Remove image paths from input
	if (images.length > 0) {
		cleanInput = input
			.replace(IMAGE_PATH_PATTERN, ' ')
			.replace(/\s+/g, ' ')
			.trim();
	}

	return Object.freeze({
		cleanInput,
		images: Object.freeze(images),
	});
}

/**
 * Format image attachment for display.
 */
export function formatImageIndicator(
	image: ImageAttachment,
	colors: { dim: (s: string) => string; cyan: (s: string) => string },
): string {
	const sizeStr = formatSize(image.size);
	return `  ${colors.cyan('[image]')} ${image.path} ${colors.dim(`(${sizeStr})`)}`;
}

function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes}B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}KB`;
	return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}
