import {
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import { join } from 'node:path';
import { Box, Text, useInput } from 'ink';
import { useCallback, useEffect, useState } from 'react';
import { TextInput } from './text-input.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface LibrarianExplorerProps {
	readonly librariansDir: string;
	readonly dataDir: string;
	readonly onDismiss: () => void;
}

interface LibrarianSummary {
	readonly name: string;
	readonly description: string;
}

interface LibrarianDefinition {
	readonly name: string;
	readonly description: string;
	readonly purpose: string;
	readonly topics: readonly string[];
	readonly permissions: {
		readonly add: boolean;
		readonly delete: boolean;
		readonly reorganize: boolean;
	};
	readonly thresholds: {
		readonly topicComplexity: number;
		readonly escalateAt: number;
	};
	readonly acp?: {
		readonly command: string;
		readonly args?: readonly string[];
		readonly agentId?: string;
	};
}

type Panel = 'list' | 'detail';
type EditMode = 'none' | 'selecting' | 'text-input' | 'confirm-delete';

interface FieldDef {
	readonly key: string;
	readonly label: string;
	readonly path: readonly string[];
	readonly type: 'string' | 'boolean' | 'number' | 'action';
	readonly presets?: readonly string[];
	readonly isHeader?: boolean;
}

// ---------------------------------------------------------------------------
// Field schema
// ---------------------------------------------------------------------------

const FIELDS: readonly FieldDef[] = Object.freeze([
	Object.freeze({
		key: 'name',
		label: 'name',
		path: ['name'] as readonly string[],
		type: 'string' as const,
	}),
	Object.freeze({
		key: 'description',
		label: 'description',
		path: ['description'] as readonly string[],
		type: 'string' as const,
	}),
	Object.freeze({
		key: 'purpose',
		label: 'purpose',
		path: ['purpose'] as readonly string[],
		type: 'string' as const,
	}),
	Object.freeze({
		key: 'topics',
		label: 'topics',
		path: ['topics'] as readonly string[],
		type: 'string' as const,
	}),
	Object.freeze({
		key: 'permissions-header',
		label: 'permissions',
		path: [] as readonly string[],
		type: 'string' as const,
		isHeader: true,
	}),
	Object.freeze({
		key: 'permissions.add',
		label: '  add',
		path: ['permissions', 'add'] as readonly string[],
		type: 'boolean' as const,
	}),
	Object.freeze({
		key: 'permissions.delete',
		label: '  delete',
		path: ['permissions', 'delete'] as readonly string[],
		type: 'boolean' as const,
	}),
	Object.freeze({
		key: 'permissions.reorganize',
		label: '  reorganize',
		path: ['permissions', 'reorganize'] as readonly string[],
		type: 'boolean' as const,
	}),
	Object.freeze({
		key: 'thresholds-header',
		label: 'thresholds',
		path: [] as readonly string[],
		type: 'string' as const,
		isHeader: true,
	}),
	Object.freeze({
		key: 'thresholds.topicComplexity',
		label: '  topicComplexity',
		path: ['thresholds', 'topicComplexity'] as readonly string[],
		type: 'number' as const,
		presets: Object.freeze(['25', '50', '100', '200']),
	}),
	Object.freeze({
		key: 'thresholds.escalateAt',
		label: '  escalateAt',
		path: ['thresholds', 'escalateAt'] as readonly string[],
		type: 'number' as const,
		presets: Object.freeze(['100', '250', '500', '1000']),
	}),
	Object.freeze({
		key: 'acp-header',
		label: 'acp',
		path: [] as readonly string[],
		type: 'string' as const,
		isHeader: true,
	}),
	Object.freeze({
		key: 'acp.command',
		label: '  command',
		path: ['acp', 'command'] as readonly string[],
		type: 'string' as const,
	}),
	Object.freeze({
		key: 'acp.args',
		label: '  args',
		path: ['acp', 'args'] as readonly string[],
		type: 'string' as const,
	}),
	Object.freeze({
		key: 'acp.agentId',
		label: '  agentId',
		path: ['acp', 'agentId'] as readonly string[],
		type: 'string' as const,
	}),
	Object.freeze({
		key: 'delete-action',
		label: '\u26A0 Delete librarian',
		path: [] as readonly string[],
		type: 'action' as const,
	}),
]);

/** Selectable fields (skip headers). */
const SELECTABLE_INDICES: readonly number[] = Object.freeze(
	FIELDS.reduce<number[]>((acc, f, i) => {
		if (!f.isHeader) acc.push(i);
		return acc;
	}, []),
);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const NAME_PATTERN = /^[a-z0-9]+(-[a-z0-9]+)*$/;

const DEFAULT_DEFINITION: LibrarianDefinition = Object.freeze({
	name: '',
	description: '',
	purpose: '',
	topics: Object.freeze(['**']),
	permissions: Object.freeze({ add: true, delete: true, reorganize: true }),
	thresholds: Object.freeze({ topicComplexity: 100, escalateAt: 500 }),
});

function loadLibrarianList(dir: string): LibrarianSummary[] {
	try {
		const files = readdirSync(dir).filter((f) => f.endsWith('.json'));
		const summaries: LibrarianSummary[] = [];
		for (const file of files) {
			try {
				const raw = readFileSync(join(dir, file), 'utf-8');
				const parsed = JSON.parse(raw) as Record<string, unknown>;
				if (typeof parsed.name === 'string') {
					summaries.push({
						name: parsed.name,
						description:
							typeof parsed.description === 'string' ? parsed.description : '',
					});
				}
			} catch {
				// Skip invalid files
			}
		}
		return summaries;
	} catch {
		return [];
	}
}

function loadDefinition(
	dir: string,
	name: string,
): LibrarianDefinition | undefined {
	try {
		const raw = readFileSync(join(dir, `${name}.json`), 'utf-8');
		return JSON.parse(raw) as LibrarianDefinition;
	} catch {
		return undefined;
	}
}

function getNestedValue(
	obj: Record<string, unknown>,
	path: readonly string[],
): unknown {
	let current: unknown = obj;
	for (const key of path) {
		if (typeof current !== 'object' || current === null) return undefined;
		current = (current as Record<string, unknown>)[key];
	}
	return current;
}

function setNestedValue(
	obj: Record<string, unknown>,
	path: readonly string[],
	value: unknown,
): Record<string, unknown> {
	const result = { ...obj };
	if (path.length === 1) {
		const key = path[0];
		if (key !== undefined) {
			result[key] = value;
		}
		return result;
	}
	if (path.length === 2) {
		const parent = path[0];
		const child = path[1];
		if (parent !== undefined && child !== undefined) {
			const existing = result[parent];
			if (typeof existing === 'object' && existing !== null) {
				result[parent] = {
					...(existing as Record<string, unknown>),
					[child]: value,
				};
			} else {
				result[parent] = { [child]: value };
			}
		}
		return result;
	}
	return result;
}

function formatFieldValue(field: FieldDef, value: unknown): string {
	if (field.key === 'topics' || field.key === 'acp.args') {
		if (Array.isArray(value)) return value.join(', ');
		return '(unset)';
	}
	if (value === undefined || value === null) return '(unset)';
	if (typeof value === 'boolean') return value ? 'true' : 'false';
	if (typeof value === 'number') return String(value);
	if (typeof value === 'string') return value || '(empty)';
	return JSON.stringify(value);
}

function truncate(str: string, maxLen: number): string {
	if (str.length <= maxLen) return str;
	return `${str.slice(0, maxLen - 1)}\u2026`;
}

function resolveAcpCommands(dataDir: string): string[] {
	try {
		const raw = readFileSync(join(dataDir, 'acp.json'), 'utf-8');
		const config = JSON.parse(raw) as Record<string, unknown>;
		const servers = Array.isArray(config.servers) ? config.servers : [];
		const commands: string[] = [];
		for (const s of servers) {
			if (
				typeof s === 'object' &&
				s !== null &&
				typeof (s as Record<string, unknown>).command === 'string'
			) {
				commands.push((s as Record<string, unknown>).command as string);
			}
		}
		return [...commands, '(unset)', 'Custom value...'];
	} catch {
		return ['(unset)', 'Custom value...'];
	}
}

/**
 * Navigate to the next selectable index in the given direction.
 */
function nextSelectableIndex(
	current: number,
	direction: 'up' | 'down',
): number {
	const idx = SELECTABLE_INDICES.indexOf(current);
	if (idx === -1) return SELECTABLE_INDICES[0] ?? 0;
	if (direction === 'up') {
		return idx > 0 ? (SELECTABLE_INDICES[idx - 1] ?? current) : current;
	}
	return idx < SELECTABLE_INDICES.length - 1
		? (SELECTABLE_INDICES[idx + 1] ?? current)
		: current;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function LibrarianExplorer({
	librariansDir,
	dataDir,
	onDismiss,
}: LibrarianExplorerProps) {
	const [panel, setPanel] = useState<Panel>('list');
	const [listIndex, setListIndex] = useState(0);
	const [fieldIndex, setFieldIndex] = useState(SELECTABLE_INDICES[0] ?? 0);
	const [editMode, setEditMode] = useState<EditMode>('none');
	const [editValue, setEditValue] = useState('');
	const [savedKey, setSavedKey] = useState<string | null>(null);
	const [isCreating, setIsCreating] = useState(false);

	// Librarian list
	const [librarians, setLibrarians] = useState<LibrarianSummary[]>(() =>
		loadLibrarianList(librariansDir),
	);

	// Current definition being edited
	const [currentDef, setCurrentDef] = useState<Record<string, unknown>>({});
	const [originalName, setOriginalName] = useState('');

	// Dropdown state
	const [dropdownOptions, setDropdownOptions] = useState<readonly string[]>([]);
	const [dropdownIndex, setDropdownIndex] = useState(0);

	// Delete confirmation state
	const [deleteSelection, setDeleteSelection] = useState(0); // 0 = Yes, 1 = No

	// Clear "Saved" indicator after a short delay
	useEffect(() => {
		if (savedKey === null) return;
		const timer = setTimeout(() => setSavedKey(null), 1500);
		return () => clearTimeout(timer);
	}, [savedKey]);

	// Total items in list: librarians + "New librarian" option
	const listItems = librarians.length + 1;

	// Reload librarian list from disk
	const reloadList = useCallback(() => {
		setLibrarians(loadLibrarianList(librariansDir));
	}, [librariansDir]);

	// Save the current definition to disk
	const saveToDisk = useCallback(
		(def: Record<string, unknown>, fieldKey: string) => {
			const name = def.name;
			if (typeof name !== 'string' || name.length === 0) return;
			if (!NAME_PATTERN.test(name)) return;

			mkdirSync(librariansDir, { recursive: true });
			const filePath = join(librariansDir, `${name}.json`);
			writeFileSync(filePath, JSON.stringify(def, null, '\t'), 'utf-8');
			setSavedKey(fieldKey);
		},
		[librariansDir],
	);

	// Handle renaming: delete old file, write new file
	const handleRename = useCallback(
		(def: Record<string, unknown>, oldName: string) => {
			const newName = def.name;
			if (typeof newName !== 'string' || newName.length === 0) return;
			if (!NAME_PATTERN.test(newName)) return;

			// Delete old file if it exists and name changed
			if (oldName && oldName !== newName) {
				const oldPath = join(librariansDir, `${oldName}.json`);
				try {
					if (existsSync(oldPath)) {
						unlinkSync(oldPath);
					}
				} catch {
					// Ignore deletion errors
				}
			}

			mkdirSync(librariansDir, { recursive: true });
			const filePath = join(librariansDir, `${newName}.json`);
			writeFileSync(filePath, JSON.stringify(def, null, '\t'), 'utf-8');
			setOriginalName(newName);
			setSavedKey('name');
		},
		[librariansDir],
	);

	// -----------------------------------------------------------------------
	// List panel input
	// -----------------------------------------------------------------------

	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
				return;
			}
			if (key.upArrow) {
				setListIndex((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setListIndex((prev) => (prev < listItems - 1 ? prev + 1 : prev));
				return;
			}
			if (key.return || key.rightArrow) {
				if (listIndex === librarians.length) {
					// New librarian
					setCurrentDef({ ...DEFAULT_DEFINITION, topics: ['**'] });
					setOriginalName('');
					setIsCreating(true);
					setFieldIndex(SELECTABLE_INDICES[0] ?? 0);
					setPanel('detail');
				} else {
					const lib = librarians[listIndex];
					if (lib) {
						const def = loadDefinition(librariansDir, lib.name);
						if (def) {
							setCurrentDef(def as unknown as Record<string, unknown>);
							setOriginalName(lib.name);
							setIsCreating(false);
							setFieldIndex(SELECTABLE_INDICES[0] ?? 0);
							setPanel('detail');
						}
					}
				}
			}
		},
		{ isActive: panel === 'list' },
	);

	// -----------------------------------------------------------------------
	// Detail panel input (browsing, not editing)
	// -----------------------------------------------------------------------

	useInput(
		(_input, key) => {
			if (key.escape || key.leftArrow) {
				setPanel('list');
				setEditMode('none');
				setIsCreating(false);
				reloadList();
				return;
			}
			if (key.upArrow) {
				setFieldIndex((prev) => nextSelectableIndex(prev, 'up'));
				return;
			}
			if (key.downArrow) {
				setFieldIndex((prev) => nextSelectableIndex(prev, 'down'));
				return;
			}
			if (key.return) {
				const field = FIELDS[fieldIndex];
				if (!field || field.isHeader) return;

				// Delete action
				if (field.key === 'delete-action') {
					const name = currentDef.name;
					if (typeof name === 'string' && name === 'default') return;
					setDeleteSelection(1); // Default to "No"
					setEditMode('confirm-delete');
					return;
				}

				// acp.command — dynamic dropdown from acp.json
				if (field.key === 'acp.command') {
					const options = resolveAcpCommands(dataDir);
					setDropdownOptions(options);
					const currentVal = getNestedValue(currentDef, field.path);
					const idx = options.indexOf(String(currentVal ?? ''));
					setDropdownIndex(idx >= 0 ? idx : 0);
					setEditMode('selecting');
					return;
				}

				// Boolean fields — dropdown
				if (field.type === 'boolean') {
					const options = ['true', 'false'];
					setDropdownOptions(options);
					const currentVal = getNestedValue(currentDef, field.path);
					setDropdownIndex(currentVal === true ? 0 : 1);
					setEditMode('selecting');
					return;
				}

				// Number fields with presets — dropdown
				if (field.type === 'number' && field.presets) {
					const options = [...field.presets, 'Custom value...'];
					setDropdownOptions(options);
					const currentVal = getNestedValue(currentDef, field.path);
					const idx = options.indexOf(String(currentVal ?? ''));
					setDropdownIndex(idx >= 0 ? idx : 0);
					setEditMode('selecting');
					return;
				}

				// String fields — text input
				if (field.type === 'string' || field.type === 'number') {
					const currentVal = getNestedValue(currentDef, field.path);
					if (field.key === 'topics' || field.key === 'acp.args') {
						// Array fields: show comma-separated
						setEditValue(
							Array.isArray(currentVal) ? currentVal.join(', ') : '',
						);
					} else {
						setEditValue(
							currentVal !== undefined && currentVal !== null
								? String(currentVal)
								: '',
						);
					}
					setEditMode('text-input');
				}
			}
		},
		{ isActive: panel === 'detail' && editMode === 'none' },
	);

	// -----------------------------------------------------------------------
	// Dropdown selector input
	// -----------------------------------------------------------------------

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
				const field = FIELDS[fieldIndex];
				if (!field) return;

				const selected = dropdownOptions[dropdownIndex];

				if (selected === 'Custom value...') {
					const currentVal = getNestedValue(currentDef, field.path);
					setEditValue(
						currentVal !== undefined && currentVal !== null
							? String(currentVal)
							: '',
					);
					setEditMode('text-input');
					return;
				}

				if (selected === '(unset)') {
					// Remove the acp.command — clear the acp section if it was the last field
					const updated = setNestedValue(currentDef, field.path, undefined);
					setCurrentDef(updated);
					saveToDisk(updated, field.key);
					setEditMode('none');
					return;
				}

				if (field.type === 'boolean') {
					const updated = setNestedValue(
						currentDef,
						field.path,
						selected === 'true',
					);
					setCurrentDef(updated);
					saveToDisk(updated, field.key);
				} else if (field.type === 'number') {
					const num = Number(selected);
					if (!Number.isNaN(num)) {
						const updated = setNestedValue(currentDef, field.path, num);
						setCurrentDef(updated);
						saveToDisk(updated, field.key);
					}
				} else {
					const updated = setNestedValue(currentDef, field.path, selected);
					setCurrentDef(updated);
					saveToDisk(updated, field.key);
				}

				setEditMode('none');
			}
		},
		{ isActive: panel === 'detail' && editMode === 'selecting' },
	);

	// -----------------------------------------------------------------------
	// Text input mode (Esc to cancel)
	// -----------------------------------------------------------------------

	useInput(
		(_input, key) => {
			if (key.escape) {
				setEditMode('none');
			}
		},
		{ isActive: panel === 'detail' && editMode === 'text-input' },
	);

	// -----------------------------------------------------------------------
	// Delete confirmation input
	// -----------------------------------------------------------------------

	useInput(
		(_input, key) => {
			if (key.escape) {
				setEditMode('none');
				return;
			}
			if (key.leftArrow || key.upArrow) {
				setDeleteSelection(0); // Yes
				return;
			}
			if (key.rightArrow || key.downArrow) {
				setDeleteSelection(1); // No
				return;
			}
			if (key.return) {
				if (deleteSelection === 0) {
					// Yes — delete the file
					const name = currentDef.name;
					if (typeof name === 'string' && name.length > 0) {
						const filePath = join(librariansDir, `${name}.json`);
						try {
							if (existsSync(filePath)) {
								unlinkSync(filePath);
							}
						} catch {
							// Ignore deletion errors
						}
					}
					setEditMode('none');
					setPanel('list');
					reloadList();
					setListIndex(0);
				} else {
					// No — cancel
					setEditMode('none');
				}
			}
		},
		{ isActive: panel === 'detail' && editMode === 'confirm-delete' },
	);

	// -----------------------------------------------------------------------
	// Submit handler for TextInput
	// -----------------------------------------------------------------------

	const handleEditSubmit = useCallback(
		(val: string) => {
			const field = FIELDS[fieldIndex];
			if (!field) return;

			const trimmed = val.trim();

			// Name field: validate kebab-case
			if (field.key === 'name') {
				if (trimmed.length === 0 || !NAME_PATTERN.test(trimmed)) {
					// Invalid name — cancel silently
					setEditMode('none');
					return;
				}
				const updated = setNestedValue(currentDef, field.path, trimmed);
				setCurrentDef(updated);
				handleRename(updated, originalName);
				if (isCreating) {
					setIsCreating(false);
				}
				setEditMode('none');
				return;
			}

			// Array fields: split by comma
			if (field.key === 'topics') {
				const items = trimmed
					.split(',')
					.map((s) => s.trim())
					.filter((s) => s.length > 0);
				const updated = setNestedValue(
					currentDef,
					field.path,
					items.length > 0 ? items : ['**'],
				);
				setCurrentDef(updated);
				saveToDisk(updated, field.key);
				setEditMode('none');
				return;
			}

			if (field.key === 'acp.args') {
				if (trimmed.length === 0) {
					const updated = setNestedValue(currentDef, field.path, undefined);
					setCurrentDef(updated);
					saveToDisk(updated, field.key);
				} else {
					const items = trimmed
						.split(',')
						.map((s) => s.trim())
						.filter((s) => s.length > 0);
					const updated = setNestedValue(currentDef, field.path, items);
					setCurrentDef(updated);
					saveToDisk(updated, field.key);
				}
				setEditMode('none');
				return;
			}

			// Number fields
			if (field.type === 'number') {
				if (trimmed.length === 0) {
					setEditMode('none');
					return;
				}
				const num = Number(trimmed);
				if (!Number.isNaN(num)) {
					const updated = setNestedValue(currentDef, field.path, num);
					setCurrentDef(updated);
					saveToDisk(updated, field.key);
				}
				setEditMode('none');
				return;
			}

			// String fields
			if (trimmed.length === 0) {
				// Empty = unset for optional acp fields
				if (field.path[0] === 'acp') {
					const updated = setNestedValue(currentDef, field.path, undefined);
					setCurrentDef(updated);
					saveToDisk(updated, field.key);
				}
			} else {
				const updated = setNestedValue(currentDef, field.path, trimmed);
				setCurrentDef(updated);
				saveToDisk(updated, field.key);
			}

			setEditMode('none');
		},
		[
			currentDef,
			fieldIndex,
			isCreating,
			originalName,
			handleRename,
			saveToDisk,
		],
	);

	// -----------------------------------------------------------------------
	// Render: List panel
	// -----------------------------------------------------------------------

	if (panel === 'list') {
		return (
			<Box flexDirection="column" paddingLeft={2} marginY={1}>
				<Text bold>Librarian Explorer</Text>
				<Text> </Text>
				{librarians.map((lib, i) => {
					const isSelected = i === listIndex;
					return (
						<Box key={lib.name}>
							<Text color={isSelected ? 'cyan' : undefined}>
								{isSelected ? '  \u276F ' : '    '}
							</Text>
							<Text bold={isSelected} color={isSelected ? 'cyan' : undefined}>
								{lib.name}
							</Text>
							<Text dimColor>
								{'  '}
								{truncate(lib.description, 50)}
							</Text>
						</Box>
					);
				})}
				<Box>
					<Text color={listIndex === librarians.length ? 'cyan' : undefined}>
						{listIndex === librarians.length ? '  \u276F ' : '    '}
					</Text>
					<Text
						italic
						bold={listIndex === librarians.length}
						color={listIndex === librarians.length ? 'cyan' : undefined}
					>
						+ New librarian...
					</Text>
				</Box>
				<Text> </Text>
				<Text dimColor>
					{'  \u2191\u2193 navigate  \u21B5/\u2192 select  esc dismiss'}
				</Text>
			</Box>
		);
	}

	// -----------------------------------------------------------------------
	// Render: Detail panel
	// -----------------------------------------------------------------------

	const defName =
		typeof currentDef.name === 'string' && currentDef.name.length > 0
			? currentDef.name
			: '(new)';
	const isDefaultLibrarian =
		typeof currentDef.name === 'string' && currentDef.name === 'default';

	return (
		<Box flexDirection="column" paddingLeft={2} marginY={1}>
			<Text bold>
				{'Librarian Explorer > '}
				<Text color="cyan">{defName}</Text>
			</Text>
			<Text> </Text>
			{FIELDS.map((field, i) => {
				const isSelected = i === fieldIndex;
				const isTextEditing = isSelected && editMode === 'text-input';
				const isDropdownOpen = isSelected && editMode === 'selecting';
				const isDeleting = isSelected && editMode === 'confirm-delete';
				const isSaved = savedKey === field.key;

				// Hide delete action for default librarian
				if (field.key === 'delete-action' && isDefaultLibrarian) {
					return null;
				}

				// Header rows
				if (field.isHeader) {
					return (
						<Box key={field.key}>
							<Text>{'    '}</Text>
							<Text dimColor bold>
								{field.label}
							</Text>
						</Box>
					);
				}

				const value = getNestedValue(currentDef, field.path);
				const displayValue = formatFieldValue(field, value);
				const isDim =
					value === undefined || value === null || displayValue === '(unset)';

				// Action row (delete)
				if (field.type === 'action') {
					return (
						<Box key={field.key} flexDirection="column">
							<Box>
								<Text color={isSelected ? 'cyan' : 'red'}>
									{isSelected ? '  \u276F ' : '    '}
								</Text>
								<Text bold={isSelected} color={isSelected ? 'cyan' : 'red'}>
									{field.label}
								</Text>
							</Box>
							{isDeleting && (
								<Box paddingLeft={6}>
									<Text>Are you sure? </Text>
									<Text
										bold={deleteSelection === 0}
										color={deleteSelection === 0 ? 'cyan' : undefined}
									>
										{deleteSelection === 0 ? '\u25B6 ' : '  '}
										Yes
									</Text>
									<Text>{'   '}</Text>
									<Text
										bold={deleteSelection === 1}
										color={deleteSelection === 1 ? 'cyan' : undefined}
									>
										{deleteSelection === 1 ? '\u25B6 ' : '  '}
										No
									</Text>
								</Box>
							)}
						</Box>
					);
				}

				return (
					<Box key={field.key} flexDirection="column">
						<Box>
							<Text color={isSelected ? 'cyan' : undefined}>
								{isSelected ? '  \u276F ' : '    '}
							</Text>
							<Text bold={isSelected} color={isSelected ? 'cyan' : undefined}>
								{field.label}
								{': '}
							</Text>
							{isTextEditing ? (
								<TextInput
									value={editValue}
									onChange={setEditValue}
									onSubmit={handleEditSubmit}
									placeholder={
										field.type === 'number'
											? 'number'
											: field.key === 'name'
												? 'kebab-case-name'
												: field.key === 'topics' || field.key === 'acp.args'
													? 'comma-separated values'
													: 'value'
									}
								/>
							) : (
								<>
									<Text
										dimColor={isDim}
										color={isSelected ? 'cyan' : undefined}
									>
										{displayValue}
									</Text>
									{isSaved && <Text color="green">{' Saved \u2713'}</Text>}
								</>
							)}
						</Box>
						{/* Inline dropdown */}
						{isDropdownOpen && (
							<Box flexDirection="column" paddingLeft={6} marginBottom={0}>
								{dropdownOptions.map((opt, oi) => {
									const isOptSelected = oi === dropdownIndex;
									return (
										<Box key={opt}>
											<Text color={isOptSelected ? 'cyan' : undefined}>
												{isOptSelected ? '\u25B6 ' : '  '}
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
			{editMode === 'confirm-delete' && (
				<Text dimColor>
					{'  \u2190\u2192 choose  \u21B5 confirm  esc cancel'}
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
