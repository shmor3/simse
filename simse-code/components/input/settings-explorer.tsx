import { join } from 'node:path';
import { Box, Text, useInput } from 'ink';
import { useCallback, useEffect, useState } from 'react';
import type {
	ConfigFileSchema,
	FieldSchema,
} from '../../features/config/settings-schema.js';
import {
	getAllConfigSchemas,
	loadConfigFile,
	saveConfigField,
} from '../../features/config/settings-schema.js';
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
type EditMode = 'none' | 'selecting' | 'text-input';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Resolves the data directory for a given config file.
 * `settings.json` lives in `<workDir>/.simse/` instead of the global dataDir.
 */
function resolveDir(
	dataDir: string,
	filename: string,
	workDir?: string,
): string {
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
 * Builds the dropdown options list for a field.
 * Enums: all options + "(unset)" + "Custom value..."
 * Booleans: true, false, (unset)
 */
function buildDropdownOptions(field: FieldSchema): readonly string[] {
	if (field.type === 'boolean') {
		return ['true', 'false', '(unset)'];
	}
	if (field.type === 'enum' && field.options) {
		return [...field.options, '(unset)', 'Custom value...'];
	}
	return [];
}

/**
 * Find the index of the current value in dropdown options.
 */
function findCurrentIndex(value: unknown, options: readonly string[]): number {
	if (value === undefined || value === null) {
		const unsetIdx = options.indexOf('(unset)');
		return unsetIdx >= 0 ? unsetIdx : 0;
	}
	const idx = options.indexOf(String(value));
	return idx >= 0 ? idx : 0;
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

	// Dropdown state
	const [dropdownOptions, setDropdownOptions] = useState<readonly string[]>([]);
	const [dropdownIndex, setDropdownIndex] = useState(0);

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
				setFileIndex((prev) => (prev < schemas.length - 1 ? prev + 1 : prev));
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

	// --- Fields panel input (browsing, not editing) ---
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
				setFieldIndex((prev) => (prev < fields.length - 1 ? prev + 1 : prev));
				return;
			}
			if (key.return) {
				const field = fields[fieldIndex];
				if (!field) return;

				if (field.type === 'boolean' || field.type === 'enum') {
					// Open dropdown selector
					const options = buildDropdownOptions(field);
					setDropdownOptions(options);
					setDropdownIndex(findCurrentIndex(fileData[field.key], options));
					setEditMode('selecting');
					return;
				}
				// string or number — enter text editing mode
				const current = fileData[field.key];
				setEditValue(
					current !== undefined && current !== null ? String(current) : '',
				);
				setEditMode('text-input');
			}
		},
		{ isActive: panel === 'fields' && editMode === 'none' },
	);

	// --- Dropdown selector input ---
	useInput(
		(_input, key) => {
			if (key.escape) {
				setEditMode('none');
				return;
			}
			if (key.upArrow) {
				setDropdownIndex((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setDropdownIndex((prev) =>
					prev < dropdownOptions.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				if (!selectedSchema) return;
				const field = selectedSchema.fields[fieldIndex];
				if (!field) return;

				const selected = dropdownOptions[dropdownIndex];
				if (selected === 'Custom value...') {
					// Switch to text input mode
					const current = fileData[field.key];
					setEditValue(
						current !== undefined && current !== null ? String(current) : '',
					);
					setEditMode('text-input');
					return;
				}
				if (selected === '(unset)') {
					saveField(selectedSchema, field.key, undefined);
				} else if (field.type === 'boolean') {
					saveField(selectedSchema, field.key, selected === 'true');
				} else {
					saveField(selectedSchema, field.key, selected);
				}
				setEditMode('none');
			}
		},
		{ isActive: panel === 'fields' && editMode === 'selecting' },
	);

	// --- Text input mode (Esc to cancel) ---
	useInput(
		(_input, key) => {
			if (key.escape) {
				setEditMode('none');
			}
		},
		{ isActive: panel === 'fields' && editMode === 'text-input' },
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
					return (
						<Box key={schema.filename}>
							<Text color={isSelected ? 'cyan' : undefined}>
								{isSelected ? '  \u276F ' : '    '}
							</Text>
							<Text bold={isSelected} color={isSelected ? 'cyan' : undefined}>
								{schema.filename.padEnd(22)}
							</Text>
							<Text dimColor>{schema.description}</Text>
						</Box>
					);
				})}
				<Text> </Text>
				<Text dimColor>
					{'  \u2191\u2193 navigate  \u21B5/\u2192 open  esc close'}
				</Text>
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
				const isTextEditing = isSelected && editMode === 'text-input';
				const isDropdownOpen = isSelected && editMode === 'selecting';
				const isSaved = savedKey === field.key;

				return (
					<Box key={field.key} flexDirection="column">
						<Box>
							<Text color={isSelected ? 'cyan' : undefined}>
								{isSelected ? '  \u276F ' : '    '}
							</Text>
							<Text bold={isSelected} color={isSelected ? 'cyan' : undefined}>
								{field.key}
								{': '}
							</Text>
							{isTextEditing ? (
								<TextInput
									value={editValue}
									onChange={setEditValue}
									onSubmit={handleEditSubmit}
									placeholder={field.type === 'number' ? 'number' : 'value'}
								/>
							) : (
								<>
									<Text
										dimColor={value === undefined || value === null}
										color={isSelected ? 'cyan' : undefined}
									>
										{formatValue(value)}
									</Text>
									{!isDropdownOpen && (
										<Text dimColor>
											{'  '}
											{field.description}
										</Text>
									)}
									{isSaved && <Text color="green">{' Saved'}</Text>}
								</>
							)}
						</Box>
						{/* Inline dropdown for enum/boolean fields */}
						{isDropdownOpen && (
							<Box flexDirection="column" paddingLeft={6} marginBottom={0}>
								{dropdownOptions.map((opt, oi) => {
									const isOptSelected = oi === dropdownIndex;
									return (
										<Box key={opt}>
											<Text color={isOptSelected ? 'cyan' : undefined}>
												{isOptSelected ? '\u276F ' : '  '}
											</Text>
											<Text
												bold={isOptSelected}
												color={isOptSelected ? 'cyan' : undefined}
												dimColor={opt === '(unset)' && !isOptSelected}
												italic={opt === 'Custom value...'}
											>
												{opt}
											</Text>
										</Box>
									);
								})}
							</Box>
						)}
					</Box>
				);
			})}
			<Text> </Text>
			{editMode === 'text-input' && (
				<Text dimColor>{'  \u21B5 save  esc cancel'}</Text>
			)}
			{editMode === 'selecting' && (
				<Text dimColor>
					{'  \u2191\u2193 navigate  \u21B5 select  esc cancel'}
				</Text>
			)}
			{editMode === 'none' && (
				<Text dimColor>
					{'  \u2191\u2193 navigate  \u21B5 edit  \u2190/esc back'}
				</Text>
			)}
		</Box>
	);
}
