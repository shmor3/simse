import { build } from 'esbuild';

await build({
	entryPoints: ['worker.ts'],
	bundle: true,
	format: 'esm',
	outfile: 'build/client/_worker.js',
	alias: {
		'virtual:react-router/server-build': './build/server/index.js',
	},
	define: {
		'import.meta.env.MODE': '"production"',
		'process.env.NODE_ENV': '"production"',
	},
	conditions: ['workerd', 'worker', 'import', 'module'],
	mainFields: ['module', 'main'],
	minify: true,
});
