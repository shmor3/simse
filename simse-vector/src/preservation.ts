// ---------------------------------------------------------------------------
// Embedding compression & text encoding utilities
// ---------------------------------------------------------------------------
//
// Pure functions for encoding/decoding embeddings (Float32 ↔ base64) and
// gzip compression of text content. No external dependencies — uses only
// node:buffer and node:zlib.
// ---------------------------------------------------------------------------

import { Buffer } from 'node:buffer';
import { gunzipSync, gzipSync } from 'node:zlib';

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface CompressionOptions {
	/**
	 * Gzip compression level (1–9). Higher = smaller output but slower.
	 * Defaults to `6` (balanced).
	 */
	readonly level?: number;
}

// ---------------------------------------------------------------------------
// Embedding encode / decode
// ---------------------------------------------------------------------------

/**
 * Encode a number[] embedding as a base64 string of Float32Array bytes.
 *
 * Reduces a 1536-dim embedding from ~30 KB (JSON number array) to
 * ~8 KB (base64 of 6144 bytes). Float32 precision is sufficient for
 * standard ML embeddings.
 */
export function encodeEmbedding(embedding: readonly number[]): string {
	const float32 = new Float32Array(embedding);
	return Buffer.from(float32.buffer).toString('base64');
}

/**
 * Decode a base64-encoded Float32Array back to a plain number[].
 */
export function decodeEmbedding(encoded: string): number[] {
	const buf = Buffer.from(encoded, 'base64');
	const float32 = new Float32Array(
		buf.buffer,
		buf.byteOffset,
		buf.byteLength / Float32Array.BYTES_PER_ELEMENT,
	);
	return Array.from(float32);
}

// ---------------------------------------------------------------------------
// Gzip text compression
// ---------------------------------------------------------------------------

/**
 * Gzip-compress a UTF-8 string.
 */
export function compressText(
	text: string,
	options?: CompressionOptions,
): Buffer {
	const level = options?.level ?? 6;
	return gzipSync(Buffer.from(text, 'utf-8'), { level });
}

/**
 * Gunzip-decompress a buffer back to a UTF-8 string.
 */
export function decompressText(data: Buffer): string {
	return gunzipSync(data).toString('utf-8');
}

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

/**
 * Detect whether a buffer starts with the gzip magic bytes (0x1f 0x8b).
 */
export function isGzipped(data: Buffer): boolean {
	return data.length >= 2 && data[0] === 0x1f && data[1] === 0x8b;
}
