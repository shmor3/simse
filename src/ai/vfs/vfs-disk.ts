// ---------------------------------------------------------------------------
// VFS Disk — commit VFS to disk / load disk into VFS
// ---------------------------------------------------------------------------

import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { join, relative, resolve, sep } from 'node:path';
import { createVFSError } from '../../errors/vfs.js';
import type { Logger } from '../../logger.js';
import { getDefaultLogger } from '../../logger.js';
import { VFS_SCHEME, toLocalPath } from './path-utils.js';
import type {
	VFSCommitOperation,
	VFSCommitOptions,
	VFSCommitResult,
	VFSLoadOptions,
} from './types.js';
import {
	createDefaultValidators,
	type VFSValidationResult,
	validateSnapshot,
} from './validators.js';
import type { VirtualFS } from './vfs.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface VFSDiskOptions {
	readonly logger?: Logger;
	readonly baseDir?: string;
}

// ---------------------------------------------------------------------------
// VFSDisk interface
// ---------------------------------------------------------------------------

export interface VFSDisk {
	readonly commit: (
		targetDir?: string,
		options?: VFSCommitOptions,
	) => Promise<VFSCommitResult>;
	readonly load: (
		sourceDir?: string,
		options?: VFSLoadOptions,
	) => Promise<VFSCommitResult>;
}

// ---------------------------------------------------------------------------
// Binary extension set
// ---------------------------------------------------------------------------

const BINARY_EXTENSIONS = new Set([
	'.png',
	'.jpg',
	'.jpeg',
	'.gif',
	'.bmp',
	'.ico',
	'.webp',
	'.svg',
	'.wasm',
	'.zip',
	'.tar',
	'.gz',
	'.bz2',
	'.xz',
	'.7z',
	'.rar',
	'.bin',
	'.exe',
	'.dll',
	'.so',
	'.dylib',
	'.dat',
	'.pdf',
	'.woff',
	'.woff2',
	'.ttf',
	'.eot',
	'.otf',
	'.mp3',
	'.mp4',
	'.webm',
	'.wav',
	'.ogg',
	'.flac',
	'.avi',
	'.mov',
	'.mkv',
	'.db',
	'.sqlite',
]);

const isBinaryPath = (filePath: string): boolean => {
	const lastDot = filePath.lastIndexOf('.');
	if (lastDot === -1) return false;
	return BINARY_EXTENSIONS.has(filePath.slice(lastDot).toLowerCase());
};

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createVFSDisk(
	vfs: VirtualFS,
	options: VFSDiskOptions = {},
): VFSDisk {
	const logger = (options.logger ?? getDefaultLogger()).child('vfs-disk');
	const baseDir = options.baseDir ?? process.cwd();

	// -- commit -----------------------------------------------------------

	const commit = async (
		targetDir?: string,
		commitOptions?: VFSCommitOptions,
	): Promise<VFSCommitResult> => {
		const resolvedTarget = resolve(targetDir ?? baseDir);
		const overwrite = commitOptions?.overwrite ?? false;
		const dryRun = commitOptions?.dryRun ?? false;
		const filter = commitOptions?.filter;

		const snap = vfs.snapshot();

		// Validate if requested
		let validation: VFSValidationResult | undefined;
		if (commitOptions?.validate) {
			const validators =
				commitOptions.validate === true
					? createDefaultValidators()
					: commitOptions.validate;
			validation = validateSnapshot(snap, validators);

			if (!validation.passed) {
				throw createVFSError(
					`Validation failed: ${validation.errors} error(s), ${validation.warnings} warning(s)`,
					{
						code: 'VFS_VALIDATION_FAILED',
						metadata: {
							errors: validation.errors,
							warnings: validation.warnings,
						},
					},
				);
			}
		}

		const operations: VFSCommitOperation[] = [];
		let filesWritten = 0;
		let directoriesCreated = 0;
		let bytesWritten = 0;

		// Create target directory
		if (!dryRun) {
			await mkdir(resolvedTarget, { recursive: true });
		}

		// Create directories (sorted by depth for correct ordering)
		const sortedDirs = [...snap.directories].sort(
			(a, b) => a.path.split('/').length - b.path.split('/').length,
		);

		for (const dir of sortedDirs) {
			if (filter && !filter(dir.path)) continue;

			const diskPath = join(
				resolvedTarget,
				...toLocalPath(dir.path).split('/').filter(Boolean),
			);

			if (!dryRun) {
				await mkdir(diskPath, { recursive: true });
			}

			operations.push(
				Object.freeze({
					type: 'mkdir' as const,
					path: dir.path,
					diskPath,
				}),
			);
			directoriesCreated++;
		}

		// Write files
		for (const file of snap.files) {
			if (filter && !filter(file.path)) continue;

			const diskPath = join(
				resolvedTarget,
				...toLocalPath(file.path).split('/').filter(Boolean),
			);

			// Check if file exists on disk
			if (!overwrite && !dryRun) {
				try {
					await stat(diskPath);
					operations.push(
						Object.freeze({
							type: 'skip' as const,
							path: file.path,
							diskPath,
							reason: 'file already exists',
						}),
					);
					continue;
				} catch {
					// File doesn't exist — proceed
				}
			}

			if (file.contentType === 'binary' && file.base64) {
				const buffer = Buffer.from(file.base64, 'base64');
				if (!dryRun) {
					// Ensure parent directory exists
					const parentDir = join(diskPath, '..');
					await mkdir(parentDir, { recursive: true });
					await writeFile(diskPath, buffer);
				}
				operations.push(
					Object.freeze({
						type: 'write' as const,
						path: file.path,
						diskPath,
						size: buffer.byteLength,
					}),
				);
				bytesWritten += buffer.byteLength;
			} else {
				const text = file.text ?? '';
				const buffer = Buffer.from(text, 'utf-8');
				if (!dryRun) {
					const parentDir = join(diskPath, '..');
					await mkdir(parentDir, { recursive: true });
					await writeFile(diskPath, text, 'utf-8');
				}
				operations.push(
					Object.freeze({
						type: 'write' as const,
						path: file.path,
						diskPath,
						size: buffer.byteLength,
					}),
				);
				bytesWritten += buffer.byteLength;
			}
			filesWritten++;
		}

		logger.debug(
			`Committed ${filesWritten} files, ${directoriesCreated} directories (${bytesWritten} bytes)${dryRun ? ' [dry run]' : ''} to "${resolvedTarget}"`,
		);

		return Object.freeze({
			filesWritten,
			directoriesCreated,
			bytesWritten,
			operations: Object.freeze(operations),
			validation,
		});
	};

	// -- load -------------------------------------------------------------

	const load = async (
		sourceDir?: string,
		loadOptions?: VFSLoadOptions,
	): Promise<VFSCommitResult> => {
		const resolvedSource = resolve(sourceDir ?? baseDir);
		const overwrite = loadOptions?.overwrite ?? false;
		const filter = loadOptions?.filter;
		const maxFileSize = loadOptions?.maxFileSize;

		// Verify source exists and is a directory
		let sourceStat: Awaited<ReturnType<typeof stat>> | undefined;
		try {
			sourceStat = await stat(resolvedSource);
		} catch (err) {
			throw createVFSError(
				`Source directory does not exist: ${resolvedSource}`,
				{
					code: 'VFS_NOT_FOUND',
					cause: err,
					metadata: { path: resolvedSource },
				},
			);
		}
		if (!sourceStat.isDirectory()) {
			throw createVFSError(`Source is not a directory: ${resolvedSource}`, {
				code: 'VFS_NOT_DIRECTORY',
				metadata: { path: resolvedSource },
			});
		}

		const operations: VFSCommitOperation[] = [];
		let filesWritten = 0;
		let directoriesCreated = 0;
		let bytesWritten = 0;

		// Recursively scan directory
		const scanDir = async (dirPath: string, vfsBase: string): Promise<void> => {
			const entries = await readdir(dirPath, { withFileTypes: true });

			for (const entry of entries) {
				const entryDiskPath = join(dirPath, entry.name);
				const entryVfsPath = `${vfsBase}/${entry.name}`;
				const relativePath = `${VFS_SCHEME}/${relative(resolvedSource, entryDiskPath).split(sep).join('/')}`;

				if (filter && !filter(relativePath)) continue;

				if (entry.isDirectory()) {
					if (!vfs.exists(entryVfsPath)) {
						vfs.mkdir(entryVfsPath, { recursive: true });
						operations.push(
							Object.freeze({
								type: 'mkdir' as const,
								path: entryVfsPath,
								diskPath: entryDiskPath,
							}),
						);
						directoriesCreated++;
					}
					await scanDir(entryDiskPath, entryVfsPath);
				} else if (entry.isFile()) {
					// Check file size
					const fileStat = await stat(entryDiskPath);
					if (maxFileSize && fileStat.size > maxFileSize) {
						operations.push(
							Object.freeze({
								type: 'skip' as const,
								path: entryVfsPath,
								diskPath: entryDiskPath,
								size: fileStat.size,
								reason: `file size ${fileStat.size} exceeds limit ${maxFileSize}`,
							}),
						);
						continue;
					}

					// Check if exists in VFS
					if (!overwrite && vfs.exists(entryVfsPath)) {
						operations.push(
							Object.freeze({
								type: 'skip' as const,
								path: entryVfsPath,
								diskPath: entryDiskPath,
								reason: 'file already exists in VFS',
							}),
						);
						continue;
					}

					const binary = isBinaryPath(entry.name);
					if (binary) {
						const data = new Uint8Array(await readFile(entryDiskPath));
						vfs.writeFile(entryVfsPath, data, {
							createParents: true,
							contentType: 'binary',
						});
						operations.push(
							Object.freeze({
								type: 'write' as const,
								path: entryVfsPath,
								diskPath: entryDiskPath,
								size: data.byteLength,
							}),
						);
						bytesWritten += data.byteLength;
					} else {
						const text = await readFile(entryDiskPath, 'utf-8');
						const size = Buffer.from(text, 'utf-8').byteLength;
						vfs.writeFile(entryVfsPath, text, {
							createParents: true,
							contentType: 'text',
						});
						operations.push(
							Object.freeze({
								type: 'write' as const,
								path: entryVfsPath,
								diskPath: entryDiskPath,
								size,
							}),
						);
						bytesWritten += size;
					}
					filesWritten++;
				}
			}
		};

		await scanDir(resolvedSource, 'vfs://');

		logger.debug(
			`Loaded ${filesWritten} files, ${directoriesCreated} directories (${bytesWritten} bytes) from "${resolvedSource}"`,
		);

		return Object.freeze({
			filesWritten,
			directoriesCreated,
			bytesWritten,
			operations: Object.freeze(operations),
		});
	};

	// -- Return frozen interface ------------------------------------------

	return Object.freeze({
		commit,
		load,
	});
}
