import { createContext, useContext } from 'react';
import type { ReactNode } from 'react';
import type { ACPClient, VFSDisk, VirtualFS } from 'simse';
import type { KnowledgeBaseApp } from '../app.js';
import type { CLIConfigResult } from '../config.js';
import type { SkillRegistry } from '../skills.js';
import type { ToolRegistry } from '../tool-registry.js';

export interface ServicesContextValue {
	readonly app: KnowledgeBaseApp;
	readonly acpClient: ACPClient;
	readonly vfs: VirtualFS;
	readonly disk: VFSDisk;
	readonly toolRegistry: ToolRegistry;
	readonly skillRegistry: SkillRegistry;
	readonly configResult: CLIConfigResult;
	readonly dataDir: string;
}

const ServicesContext = createContext<ServicesContextValue | null>(null);

export function useServices(): ServicesContextValue {
	const ctx = useContext(ServicesContext);
	if (!ctx) throw new Error('useServices must be used within a ServicesProvider');
	return ctx;
}

interface ServicesProviderProps {
	readonly value: ServicesContextValue;
	readonly children: ReactNode;
}

export function ServicesProvider({ value, children }: ServicesProviderProps) {
	return (
		<ServicesContext.Provider value={value}>
			{children}
		</ServicesContext.Provider>
	);
}
