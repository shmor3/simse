//! Shared PTY test helpers for simse-tui integration tests.
//!
//! All tests spawn the real `simse-tui` binary in a pseudo-terminal via
//! `portable-pty` + `vt100::Parser`. On Windows, PTY reads are blocking,
//! so we use a dedicated reader thread that feeds output into a shared
//! `vt100::Parser`. This provides a complete VT terminal emulator that
//! correctly handles erase sequences, cursor movement, scrolling, etc.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
pub use ratatui_testlib::{KeyCode, Modifiers};

/// Custom test harness that uses a background reader thread.
///
/// Wraps a `portable-pty` master handle and a shared `vt100::Parser`.
/// A background thread continuously reads from the PTY master and feeds
/// data into the parser, so `wait_for_text` never blocks.
pub struct PtyHarness {
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    parser: Arc<Mutex<vt100_ctt::Parser>>,
    _reader_handle: std::thread::JoinHandle<()>,
    // Keep the PtyPair alive so the ConPTY handle is not closed.
    // On Windows, dropping the PtyPair closes the ConPTY, terminating
    // the child process. We must hold it as long as the child runs.
    _pty_pair: PtyPair,
    timeout: Duration,
}

impl PtyHarness {
    /// Spawn a command in a new PTY with the given dimensions and timeout.
    pub fn spawn(cmd: CommandBuilder, width: u16, height: u16, timeout: Duration) -> Self {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: height,
                cols: width,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("Failed to open PTY");

        let child = pair
            .slave
            .spawn_command(cmd)
            .expect("Failed to spawn command in PTY");

        // Get separate reader and writer handles from the master side.
        let mut reader = pair
            .master
            .try_clone_reader()
            .expect("Failed to clone PTY reader");
        let writer = pair
            .master
            .take_writer()
            .expect("Failed to take PTY writer");

        let parser = Arc::new(Mutex::new(vt100_ctt::Parser::new(height, width, 0)));
        let parser_bg = Arc::clone(&parser);

        // Background thread: read PTY output and feed into vt100 parser.
        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if let Ok(mut p) = parser_bg.lock() {
                            p.process(&buf[..n]);
                        }
                    }
                    Err(e) => {
                        let kind = e.kind();
                        if kind == std::io::ErrorKind::BrokenPipe
                            || kind == std::io::ErrorKind::UnexpectedEof
                        {
                            break;
                        }
                        if kind == std::io::ErrorKind::WouldBlock {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        // Other error — stop reading.
                        break;
                    }
                }
            }
        });

        Self {
            writer,
            child,
            parser,
            _reader_handle: reader_handle,
            _pty_pair: pair,
            timeout,
        }
    }

    /// Wait for specific text to appear on screen, with timeout.
    pub fn wait_for_text(&self, text: &str) -> Result<(), String> {
        let start = Instant::now();
        let poll = Duration::from_millis(50);

        loop {
            if let Ok(p) = self.parser.lock() {
                let contents = p.screen().contents();
                if contents.contains(text) {
                    return Ok(());
                }
            }
            if start.elapsed() >= self.timeout {
                let contents = self.screen_contents();
                return Err(format!(
                    "Timeout after {:?} waiting for text '{}'. Screen contents:\n{}",
                    self.timeout, text, contents
                ));
            }
            std::thread::sleep(poll);
        }
    }

    /// Return current screen contents as a string.
    pub fn screen_contents(&self) -> String {
        self.parser
            .lock()
            .map(|p| p.screen().contents())
            .unwrap_or_default()
    }

    /// Send a string of characters to the PTY.
    pub fn send_keys(&mut self, text: &str) -> Result<(), String> {
        self.writer
            .write_all(text.as_bytes())
            .map_err(|e| format!("Failed to send keys: {e}"))?;
        self.writer
            .flush()
            .map_err(|e| format!("Failed to flush: {e}"))
    }

    /// Send a single key code.
    pub fn send_key(&mut self, key: KeyCode) -> Result<(), String> {
        let bytes = encode_key(key, Modifiers::empty());
        self.writer
            .write_all(&bytes)
            .map_err(|e| format!("Failed to send key: {e}"))?;
        self.writer
            .flush()
            .map_err(|e| format!("Failed to flush: {e}"))
    }

    /// Check if the child process is still running.
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) => false, // exited
            Ok(None) => true,     // still running
            Err(_) => false,      // treat error as exited
        }
    }

    /// Send a key with modifiers.
    pub fn send_key_with_modifiers(
        &mut self,
        key: KeyCode,
        mods: Modifiers,
    ) -> Result<(), String> {
        let bytes = encode_key(key, mods);
        self.writer
            .write_all(&bytes)
            .map_err(|e| format!("Failed to send key: {e}"))?;
        self.writer
            .flush()
            .map_err(|e| format!("Failed to flush: {e}"))
    }
}

impl Drop for PtyHarness {
    fn drop(&mut self) {
        // Kill the child so the reader thread's blocking read unblocks.
        let _ = self.child.kill();
    }
}

/// Encode a key + modifiers into bytes suitable for a terminal.
fn encode_key(key: KeyCode, mods: Modifiers) -> Vec<u8> {
    match key {
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Esc => b"\x1b".to_vec(),
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::Backspace => {
            if mods.contains(Modifiers::ALT) {
                // Alt+Backspace = ESC + DEL
                b"\x1b\x7f".to_vec()
            } else {
                b"\x7f".to_vec()
            }
        }
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Char(c) => {
            if mods.contains(Modifiers::CTRL) {
                // Ctrl+letter = letter & 0x1f
                let ctrl = (c as u8) & 0x1f;
                vec![ctrl]
            } else {
                let mut buf = [0u8; 4];
                c.encode_utf8(&mut buf).as_bytes().to_vec()
            }
        }
        _ => Vec::new(),
    }
}

// ── Public helpers ──────────────────────────────────────────

/// Spawn the real simse-tui binary with an isolated data directory.
pub fn spawn_simse(data_dir: &Path) -> PtyHarness {
    let binary = env!("CARGO_BIN_EXE_simse-tui");
    let mut cmd = CommandBuilder::new(binary);
    cmd.arg("--data-dir");
    cmd.arg(data_dir.to_str().expect("data_dir must be valid UTF-8"));
    PtyHarness::spawn(cmd, 120, 40, Duration::from_secs(10))
}

/// Spawn simse-tui with an explicit working directory.
pub fn spawn_simse_with_cwd(data_dir: &Path, work_dir: &Path) -> PtyHarness {
    let binary = env!("CARGO_BIN_EXE_simse-tui");
    let mut cmd = CommandBuilder::new(binary);
    cmd.arg("--data-dir");
    cmd.arg(data_dir.to_str().expect("data_dir must be valid UTF-8"));
    cmd.cwd(work_dir);
    PtyHarness::spawn(cmd, 120, 40, Duration::from_secs(15))
}

/// Write default config files (config.json + acp.json) to a data directory.
pub fn write_default_config(data_dir: &Path) {
    std::fs::create_dir_all(data_dir).unwrap();
    std::fs::write(
        data_dir.join("config.json"),
        r#"{"logLevel": "warn"}"#,
    )
    .unwrap();
    std::fs::write(
        data_dir.join("acp.json"),
        r#"{"servers": [{"name": "claude-code", "command": "claude"}]}"#,
    )
    .unwrap();
}

/// Spawn simse-tui with a pre-configured data directory.
pub fn spawn_simse_configured(data_dir: &Path) -> PtyHarness {
    write_default_config(data_dir);
    spawn_simse(data_dir)
}

/// Type a command string and press Enter.
pub fn type_command(h: &mut PtyHarness, cmd: &str) {
    h.send_keys(cmd).expect("Failed to send keys");
    h.send_key(KeyCode::Enter).expect("Failed to send Enter");
}

/// Send Ctrl+C.
pub fn send_ctrl_c(h: &mut PtyHarness) {
    h.send_key_with_modifiers(KeyCode::Char('c'), Modifiers::CTRL)
        .expect("Failed to send Ctrl+C");
}

/// Send Ctrl+L.
pub fn send_ctrl_l(h: &mut PtyHarness) {
    h.send_key_with_modifiers(KeyCode::Char('l'), Modifiers::CTRL)
        .expect("Failed to send Ctrl+L");
}

/// Send Escape.
pub fn send_escape(h: &mut PtyHarness) {
    h.send_key(KeyCode::Esc).expect("Failed to send Escape");
}

/// Wait for the app to fully start (banner visible).
pub fn wait_for_startup(h: &PtyHarness) {
    h.wait_for_text("simse v")
        .expect("App did not start — 'simse v' not found on screen");
}
