import { Box, Text, useInput } from 'ink';
import { useCallback, useEffect, useState } from 'react';
import { join } from 'node:path';
import {
	getAllConfigSchemas,
	loadConfigFile,
	saveConfigField,
} from '../../features/config/settings-schema.js';
import type { ConfigFileSchema } from '../../features/config/settings-schema.js';
import { TextInput } from './text-input.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface SettingsExplorerProps {
	readonly dataDir: string;
	readonly workDir?: string;
	readonly onDismiss: () => void;
}

type Panel = 'files' | 'fields';
type EditMode = 'none' | 'editing';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Resolves the data directory for a given config file.
 * `settings.json` lives in `<workDir>/.simse/` instead of the global dataDir.
 */
function resolveDir(dataDir: string, filename: string, workDir?: string): string {
	if (filename === 'settings.json') {
		return join(workDir ?? process.cwd(), '.simse');
	}
	return dataDir;
}

/**
 * Formats a field value for display.
 */
function formatValue(value: unknown): string {
	if (value === undefined || value === null) return '(not set)';
	if (typeof value === 'boolean') return value ? 'true' : 'false';
	if (typeof value === 'number') return String(value);
	if (typeof value === 'string') return value || '(empty)';
	return JSON.stringify(value);
}

/**
 * Returns the next value in a boolean toggle cycle: true -> false -> undefined (unset).
 */
function toggleBoolean(current: unknown): boolean | undefined {
	if (current === true) return false;
	if (current === false) return undefined;
	return true;
}

/**
 * Cycles through enum options + undefined (unset).
 */
function cycleEnum(current: unknown, options: readonly string[]): string | undefined {
	if (options.length === 0) return undefined;
	if (current === undefined || current === null) return options[0];
	const idx = options.indexOf(String(current));
	if (idx === -1 || idx === options.length - 1) return undefined;
	return options[idx + 1];
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function SettingsExplorer({
	dataDir,
	workDir,
	onDismiss,
}: SettingsExplorerProps) {
	const schemas = getAllConfigSchemas();

	const [panel, setPanel] = useState<Panel>('files');
	const [fileIndex, setFileIndex] = useState(0);
	const [fieldIndex, setFieldIndex] = useState(0);
	const [editMode, setEditMode] = useState<EditMode>('none');
	const [editValue, setEditValue] = useState('');
	const [fileData, setFileData] = useState<Record<string, unknown>>({});
	const [savedKey, setSavedKey] = useState<string | null>(null);

	const selectedSchema = schemas[fileIndex] as ConfigFileSchema | undefined;

	// Clear "Saved" indicator after a short delay
	useEffect(() => {
		if (savedKey === null) return;
		const timer = setTimeout(() => setSavedKey(null), 1500);
		return () => clearTimeout(timer);
	}, [savedKey]);

	// Load file data when entering the fields panel
	const loadFile = useCallback(
		(schema: ConfigFileSchema) => {
			const dir = resolveDir(dataDir, schema.filename, workDir);
			setFileData(loadConfigFile(dir, schema.filename));
		},
		[dataDir, workDir],
	);

	// Save a field value, reload data, and show indicator
	const saveField = useCallback(
		(schema: ConfigFileSchema, key: string, value: unknown) => {
			const dir = resolveDir(dataDir, schema.filename, workDir);
			saveConfigField(dir, schema.filename, key, value);
			setFileData(loadConfigFile(dir, schema.filename));
			setSavedKey(key);
		},
		[dataDir, workDir],
	);

	// --- Files panel input ---
	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
				return;
			}
			if (key.upArrow) {
				setFileIndex((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setFileIndex((prev) =>
					prev < schemas.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return || key.rightArrow) {
				const schema = schemas[fileIndex];
				if (schema && schema.fields.length > 0) {
					loadFile(schema);
					setFieldIndex(0);
					setPanel('fields');
				}
			}
		},
		{ isActive: panel === 'files' },
	);

	// --- Fields panel input (not editing) ---
	useInput(
		(_input, key) => {
			if (!selectedSchema) return;
			const fields = selectedSchema.fields;

			if (key.escape || key.leftArrow) {
				setPanel('files');
				setEditMode('none');
				return;
			}
			if (key.upArrow) {
				setFieldIndex((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setFieldIndex((prev) =>
					prev < fields.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				const field = fields[fieldIndex];
				if (!field) return;

				if (field.type === 'boolean') {
					const next = toggleBoolean(fileData[field.key]);
					saveField(selectedSchema, field.key, next);
					return;
				}
				if (field.type === 'enum' && field.options) {
					const next = cycleEnum(fileData[field.key], field.options);
					saveField(selectedSchema, field.key, next);
					return;
				}
				// string or number — enter text editing mode
				const current = fileData[field.key];
				setEditValue(
					current !== undefined && current !== null ? String(current) : '',
				);
				setEditMode('editing');
			}
		},
		{ isActive: panel === 'fields' && editMode === 'none' },
	);

	// --- Editing mode input (Esc to cancel) ---
	useInput(
		(_input, key) => {
			if (key.escape) {
				setEditMode('none');
			}
		},
		{ isActive: panel === 'fields' && editMode === 'editing' },
	);

	// --- Submit handler for TextInput ---
	const handleEditSubmit = useCallback(
		(val: string) => {
			if (!selectedSchema) return;
			const field = selectedSchema.fields[fieldIndex];
			if (!field) return;

			const trimmed = val.trim();

			if (trimmed === '') {
				// Empty = unset
				saveField(selectedSchema, field.key, undefined);
			} else if (field.type === 'number') {
				const num = Number(trimmed);
				if (!Number.isNaN(num)) {
					saveField(selectedSchema, field.key, num);
				}
				// If invalid number, just cancel silently
			} else {
				saveField(selectedSchema, field.key, trimmed);
			}

			setEditMode('none');
		},
		[selectedSchema, fieldIndex, saveField],
	);

	// -----------------------------------------------------------------------
	// Render: Files panel
	// -----------------------------------------------------------------------

	if (panel === 'files') {
		return (
			<Box flexDirection="column" paddingLeft={2} marginY={1}>
				<Text bold>Settings Explorer</Text>
				<Text> </Text>
				{schemas.map((schema, i) => {
					const isSelected = i === fileIndex;
					const hasFields = schema.fields.length > 0;
					return (
						<Box key={schema.filename}>
							<Text color={isSelected ? 'cyan' : undefined}>
								{isSelected ? '  \u276F ' : '    '}
							</Text>
							<Text
								bold={isSelected}
								color={isSelected ? 'cyan' : undefined}
								dimColor={!hasFields && !isSelected}
							>
								{schema.filename.padEnd(22)}
							</Text>
							<Text dimColor>{schema.description}</Text>
						</Box>
					);
				})}
				<Text> </Text>
				<Text dimColor>{'  \u2191\u2193 navigate  \u21B5/\u2192 open  esc close'}</Text>
			</Box>
		);
	}

	// -----------------------------------------------------------------------
	// Render: Fields panel
	// -----------------------------------------------------------------------

	if (!selectedSchema) return null;
	const fields = selectedSchema.fields;

	return (
		<Box flexDirection="column" paddingLeft={2} marginY={1}>
			<Text bold>
				{'Settings Explorer > '}
				<Text color="cyan">{selectedSchema.filename}</Text>
			</Text>
			<Text> </Text>
			{fields.map((field, i) => {
				const isSelected = i === fieldIndex;
				const value = fileData[field.key];
				const isEditing = isSelected && editMode === 'editing';
				const isSaved = savedKey === field.key;

				return (
					<Box key={field.key}>
						<Text color={isSelected ? 'cyan' : undefined}>
							{isSelected ? '  \u276F ' : '    '}
						</Text>
						<Text
							bold={isSelected}
							color={isSelected ? 'cyan' : undefined}
						>
							{field.key}
							{': '}
						</Text>
						{isEditing ? (
							<TextInput
								value={editValue}
								onChange={setEditValue}
								onSubmit={handleEditSubmit}
								placeholder={
									field.type === 'number' ? 'number' : 'value'
								}
							/>
						) : (
							<>
								<Text
									dimColor={value === undefined || value === null}
									color={isSelected ? 'cyan' : undefined}
								>
									{formatValue(value)}
								</Text>
								<Text dimColor>
									{'  '}
									{field.description}
								</Text>
								{isSaved && (
									<Text color="green">{' Saved'}</Text>
								)}
							</>
						)}
					</Box>
				);
			})}
			<Text> </Text>
			{editMode === 'editing' ? (
				<Text dimColor>
					{'  \u21B5 save  esc cancel'}
				</Text>
			) : (
				<Text dimColor>
					{'  \u2191\u2193 navigate  \u21B5 edit  \u2190/esc back'}
				</Text>
			)}
		</Box>
	);
}
