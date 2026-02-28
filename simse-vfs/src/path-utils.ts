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

export const VFS_SCHEME = 'vfs://';

export const VFS_ROOT = `${VFS_SCHEME}/`;

export const toLocalPath = (vfsPath: string): string => {
	if (!vfsPath.startsWith(VFS_SCHEME)) {
		throw new Error(`Path must start with ${VFS_SCHEME}: ${vfsPath}`);
	}
	return vfsPath.slice(VFS_SCHEME.length) || '/';
};

export const normalizePath = (input: string): string => {
	if (!input.startsWith(VFS_SCHEME)) {
		throw new Error(`Path must start with ${VFS_SCHEME}: ${input}`);
	}

	let p = input.slice(VFS_SCHEME.length).replace(/\\/g, '/');
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

	return resolved.length === 0
		? `${VFS_SCHEME}/`
		: `${VFS_SCHEME}/${resolved.join('/')}`;
};

export const parentPath = (normalizedPath: string): string | undefined => {
	if (normalizedPath === VFS_ROOT) return undefined;
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	const lastSlash = localPart.lastIndexOf('/');
	if (lastSlash === 0) return VFS_ROOT;
	return `${VFS_SCHEME}${localPart.slice(0, lastSlash)}`;
};

export const baseName = (normalizedPath: string): string => {
	if (normalizedPath === VFS_ROOT) return '';
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	const lastSlash = localPart.lastIndexOf('/');
	return localPart.slice(lastSlash + 1);
};

export const ancestorPaths = (normalizedPath: string): string[] => {
	const result: string[] = [VFS_ROOT];
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	const segments = localPart.split('/').filter(Boolean);
	for (let i = 0; i < segments.length - 1; i++) {
		result.push(`${VFS_SCHEME}/${segments.slice(0, i + 1).join('/')}`);
	}
	return result;
};

export const pathDepth = (normalizedPath: string): number => {
	if (normalizedPath === VFS_ROOT) return 0;
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	return localPart.split('/').filter(Boolean).length;
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
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	if (localPart.length > limits.maxPathLength) {
		return `Path exceeds max length (${limits.maxPathLength})`;
	}
	const depth = pathDepth(normalizedPath);
	if (depth > limits.maxPathDepth) {
		return `Path exceeds max depth (${limits.maxPathDepth})`;
	}
	const segments = localPart.split('/').filter(Boolean);
	for (const seg of segments) {
		const segError = validateSegment(seg, limits);
		if (segError) return segError;
	}
	return undefined;
};
