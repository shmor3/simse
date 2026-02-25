// ---------------------------------------------------------------------------
// VFS Path Utilities — pure functions for path manipulation
// ---------------------------------------------------------------------------

import type { VFSLimits } from './types.js';

const hasForbiddenChars = (segment: string): boolean => {
	for (let i = 0; i < segment.length; i++) {
		const code = segment.charCodeAt(i);
		if (code <= 0x1f || segment[i] === '\\') return true;
	}
	return false;
};

export const normalizePath = (input: string): string => {
	let p = input.replace(/\\/g, '/');
	if (!p.startsWith('/')) p = `/${p}`;

	const segments = p.split('/');
	const resolved: string[] = [];

	for (const seg of segments) {
		if (seg === '' || seg === '.') continue;
		if (seg === '..') {
			resolved.pop();
		} else {
			resolved.push(seg);
		}
	}

	return resolved.length === 0 ? '/' : `/${resolved.join('/')}`;
};

export const parentPath = (normalizedPath: string): string | undefined => {
	if (normalizedPath === '/') return undefined;
	const lastSlash = normalizedPath.lastIndexOf('/');
	return lastSlash === 0 ? '/' : normalizedPath.slice(0, lastSlash);
};

export const baseName = (normalizedPath: string): string => {
	if (normalizedPath === '/') return '';
	const lastSlash = normalizedPath.lastIndexOf('/');
	return normalizedPath.slice(lastSlash + 1);
};

export const ancestorPaths = (normalizedPath: string): string[] => {
	const result: string[] = ['/'];
	const segments = normalizedPath.split('/').filter(Boolean);
	for (let i = 0; i < segments.length - 1; i++) {
		result.push(`/${segments.slice(0, i + 1).join('/')}`);
	}
	return result;
};

export const pathDepth = (normalizedPath: string): number => {
	if (normalizedPath === '/') return 0;
	return normalizedPath.split('/').filter(Boolean).length;
};

export const validateSegment = (
	segment: string,
	limits: Required<VFSLimits>,
): string | undefined => {
	if (segment.length === 0) return 'Path segment cannot be empty';
	if (segment.length > limits.maxNameLength) {
		return `Path segment exceeds max name length (${limits.maxNameLength})`;
	}
	if (hasForbiddenChars(segment)) {
		return 'Path segment contains forbidden characters';
	}
	return undefined;
};

export const validatePath = (
	normalizedPath: string,
	limits: Required<VFSLimits>,
): string | undefined => {
	if (normalizedPath.length > limits.maxPathLength) {
		return `Path exceeds max length (${limits.maxPathLength})`;
	}
	const depth = pathDepth(normalizedPath);
	if (depth > limits.maxPathDepth) {
		return `Path exceeds max depth (${limits.maxPathDepth})`;
	}
	const segments = normalizedPath.split('/').filter(Boolean);
	for (const seg of segments) {
		const segError = validateSegment(seg, limits);
		if (segError) return segError;
	}
	return undefined;
};
