/**
 * Node.js bridge for node-pty.
 *
 * Bun has a compatibility issue with node-pty's write pipe on Windows
 * (ERR_SOCKET_CLOSED). This bridge runs under Node.js where node-pty
 * works correctly, and communicates with the Bun test process via
 * newline-delimited JSON over stdin/stdout.
 *
 * Protocol:
 *   Parent → Bridge (stdin):
 *     { "type": "write", "data": "..." }
 *     { "type": "kill" }
 *     { "type": "resize", "cols": N, "rows": N }
 *
 *   Bridge → Parent (stdout):
 *     { "type": "data", "data": "..." }
 *     { "type": "exit", "exitCode": N }
 *     { "type": "ready" }
 */

const pty = require('node-pty');

const config = JSON.parse(process.argv[2] || '{}');

const proc = pty.spawn(config.command || 'cmd.exe', config.args || [], {
	cols: config.cols || 120,
	rows: config.rows || 40,
	cwd: config.cwd || process.cwd(),
	env: { ...process.env, ...(config.env || {}) },
});

function send(msg) {
	process.stdout.write(`${JSON.stringify(msg)}\n`);
}

proc.onData((data) => {
	send({ type: 'data', data });
});

proc.onExit(({ exitCode }) => {
	send({ type: 'exit', exitCode });
	// Give the parent time to read the exit message
	setTimeout(() => process.exit(0), 100);
});

send({ type: 'ready', pid: proc.pid });

let buf = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {
	buf += chunk;
	let nl = buf.indexOf('\n');
	while (nl >= 0) {
		const line = buf.slice(0, nl);
		buf = buf.slice(nl + 1);
		if (!line.trim()) continue;
		try {
			const msg = JSON.parse(line);
			if (msg.type === 'write') {
				proc.write(msg.data);
			} else if (msg.type === 'kill') {
				try {
					proc.kill();
				} catch {
					/* ignore */
				}
				setTimeout(() => process.exit(0), 100);
			} else if (msg.type === 'resize') {
				proc.resize(msg.cols, msg.rows);
			}
		} catch {
			// Ignore parse errors
		}
		nl = buf.indexOf('\n');
	}
});

process.stdin.resume();
