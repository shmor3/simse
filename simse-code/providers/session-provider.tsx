import { createContext, useCallback, useContext, useState } from 'react';
import type { ReactNode } from 'react';
import type { PermissionMode } from '../ink-types.js';
import type { Conversation } from '../conversation.js';

interface SessionContextValue {
	readonly serverName: string | undefined;
	readonly agentName: string | undefined;
	readonly libraryEnabled: boolean;
	readonly bypassPermissions: boolean;
	readonly maxTurns: number;
	readonly totalTurns: number;
	readonly permissionMode: PermissionMode;
	readonly planMode: boolean;
	readonly verbose: boolean;
	readonly conversation: Conversation | undefined;
	readonly abortController: AbortController | undefined;

	// Setters
	readonly setServerName: (name: string | undefined) => void;
	readonly setAgentName: (name: string | undefined) => void;
	readonly setLibraryEnabled: (enabled: boolean) => void;
	readonly setBypassPermissions: (bypass: boolean) => void;
	readonly setMaxTurns: (turns: number) => void;
	readonly incrementTurns: () => void;
	readonly setPermissionMode: (mode: PermissionMode) => void;
	readonly setPlanMode: (active: boolean) => void;
	readonly setVerbose: (verbose: boolean) => void;
	readonly setConversation: (conv: Conversation) => void;
	readonly setAbortController: (ctrl: AbortController | undefined) => void;
}

const SessionContext = createContext<SessionContextValue | null>(null);

export function useSession(): SessionContextValue {
	const ctx = useContext(SessionContext);
	if (!ctx) throw new Error('useSession must be used within a SessionProvider');
	return ctx;
}

interface SessionProviderProps {
	readonly children: ReactNode;
	readonly initialServerName?: string;
	readonly initialAgentName?: string;
	readonly initialLibraryEnabled?: boolean;
	readonly initialBypassPermissions?: boolean;
	readonly initialMaxTurns?: number;
	readonly initialConversation?: Conversation;
}

export function SessionProvider({
	children,
	initialServerName,
	initialAgentName,
	initialLibraryEnabled = true,
	initialBypassPermissions = false,
	initialMaxTurns = 10,
	initialConversation,
}: SessionProviderProps) {
	const [serverName, setServerName] = useState(initialServerName);
	const [agentName, setAgentName] = useState(initialAgentName);
	const [libraryEnabled, setLibraryEnabled] = useState(initialLibraryEnabled);
	const [bypassPermissions, setBypassPermissions] = useState(initialBypassPermissions);
	const [maxTurns, setMaxTurns] = useState(initialMaxTurns);
	const [totalTurns, setTotalTurns] = useState(0);
	const [permissionMode, setPermissionMode] = useState<PermissionMode>('default');
	const [planMode, setPlanMode] = useState(false);
	const [verbose, setVerbose] = useState(false);
	const [conversation, setConversation] = useState<Conversation | undefined>(initialConversation);
	const [abortController, setAbortController] = useState<AbortController | undefined>();

	const incrementTurns = useCallback(() => setTotalTurns((t) => t + 1), []);

	const value: SessionContextValue = {
		serverName, agentName, libraryEnabled, bypassPermissions,
		maxTurns, totalTurns, permissionMode, planMode, verbose,
		conversation, abortController,
		setServerName, setAgentName, setLibraryEnabled, setBypassPermissions,
		setMaxTurns, incrementTurns, setPermissionMode, setPlanMode,
		setVerbose, setConversation, setAbortController,
	};

	return (
		<SessionContext.Provider value={value}>{children}</SessionContext.Provider>
	);
}
