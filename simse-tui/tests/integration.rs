//! Integration tests for simse-tui.
//!
//! These are "integration-lite" tests that exercise multiple modules working
//! together. They use `ratatui::backend::TestBackend` for rendering tests and
//! the Elm-architecture `update` function for state machine tests.

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use simse_tui::app::{update, view, App, AppMessage, LoopStatus, Screen};
use simse_tui::autocomplete::CommandAutocompleteState;
use simse_tui::cli_args::{parse_cli_args, CliArgs};
use simse_tui::dialogs::confirm::ConfirmDialogState;
use simse_tui::dispatch::{dispatch_command, parse_command_line, DispatchContext};
use simse_tui::commands::{BridgeAction, CommandContext, CommandOutput, OverlayAction};
use simse_tui::overlays::settings::{SettingsExplorerState, SettingsLevel, CONFIG_FILES};
use simse_ui_core::app::{
    OutputItem, PermissionOption, PermissionRequest, ToolCallState, ToolCallStatus,
};
use simse_ui_core::commands::registry::all_commands;
use simse_ui_core::input::state as input;

// ═══════════════════════════════════════════════════════════════
// 1. App startup -> banner visible
// ═══════════════════════════════════════════════════════════════

#[test]
fn app_startup_renders_banner() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let app = App::new();

    assert!(app.banner_visible);
    assert!(app.output.is_empty());
    assert_eq!(app.screen, Screen::Chat);
    assert_eq!(app.loop_status, LoopStatus::Idle);

    // Render should not panic and should produce output.
    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();

    // Verify the buffer contains the version string somewhere.
    let buffer = terminal.backend().buffer().clone();
    let content: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(
        content.contains("simse"),
        "Banner should contain 'simse' in the rendered output"
    );
}

#[test]
fn app_startup_shows_tips_in_banner() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let app = App::new();

    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(
        content.contains("Tips"),
        "Banner should contain 'Tips' section"
    );
}

#[test]
fn app_startup_status_bar_shows_permission_mode() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let app = App::new();

    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(
        content.contains("ask"),
        "Status bar should show default permission mode 'ask'"
    );
}

// ═══════════════════════════════════════════════════════════════
// 2. Submit text -> appears in output
// ═══════════════════════════════════════════════════════════════

#[test]
fn submit_user_text_appears_in_output() {
    let mut app = App::new();

    // Type "hello world" and submit.
    app.input = input::insert(&app.input, "hello world");
    app = update(app, AppMessage::Submit);

    // Banner should be hidden after first submission.
    assert!(!app.banner_visible);

    // Output should contain the user message.
    assert_eq!(app.output.len(), 1);
    match &app.output[0] {
        OutputItem::Message { role, text } => {
            assert_eq!(role, "user");
            assert_eq!(text, "hello world");
        }
        other => panic!("Expected Message, got {:?}", other),
    }

    // History should contain the submitted text.
    assert_eq!(app.history, vec!["hello world"]);

    // Input should be cleared.
    assert!(app.input.value.is_empty());
}

#[test]
fn submit_renders_user_message_with_chevron() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new();
    app.input = input::insert(&app.input, "test message");
    app = update(app, AppMessage::Submit);

    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(
        content.contains("test message"),
        "Rendered output should contain the user's message"
    );
}

#[test]
fn submit_multiple_messages_all_appear() {
    let mut app = App::new();

    app.input = input::insert(&app.input, "first message");
    app = update(app, AppMessage::Submit);

    app.input = input::insert(&app.input, "second message");
    app = update(app, AppMessage::Submit);

    app.input = input::insert(&app.input, "third message");
    app = update(app, AppMessage::Submit);

    assert_eq!(app.output.len(), 3);
    assert_eq!(app.history.len(), 3);
}

// ═══════════════════════════════════════════════════════════════
// 3. Command dispatch -> all categories produce correct output
// ═══════════════════════════════════════════════════════════════

#[test]
fn dispatch_library_commands_produce_output() {
    let library_cmds = [
        ("add", "topic some text"),
        ("search", "query"),
        ("recommend", "patterns"),
        ("topics", ""),
        ("volumes", "rust"),
        ("get", "id-1"),
        ("delete", "id-2"),
    ];

    for (cmd, args) in &library_cmds {
        let out = dispatch_command(cmd, args);
        assert!(
            out.len() >= 2,
            "Library command /{cmd} should produce at least 2 outputs (Info + BridgeRequest)"
        );
        // Library commands now return Info feedback then BridgeRequest.
        assert!(
            matches!(&out[0], CommandOutput::Info(_)),
            "Library command /{cmd} should return Info feedback first, got {:?}",
            out[0]
        );
        assert!(
            matches!(&out[1], CommandOutput::BridgeRequest(_)),
            "Library command /{cmd} should return BridgeRequest second, got {:?}",
            out[1]
        );
    }
}

#[test]
fn dispatch_librarians_opens_overlay() {
    let out = dispatch_command("librarians", "");
    assert!(matches!(
        &out[0],
        CommandOutput::OpenOverlay(OverlayAction::Librarians)
    ));
}

#[test]
fn dispatch_session_commands_produce_output() {
    let session_cmds = [
        ("sessions", ""),
        ("resume", "sess-1"),
        ("rename", "New Name"),
        ("server", "ollama"),
        ("model", "gpt-4o"),
        ("mcp", "status"),
        ("acp", "restart"),
    ];

    for (cmd, args) in &session_cmds {
        let out = dispatch_command(cmd, args);
        assert!(
            !out.is_empty(),
            "Session command /{cmd} should produce output"
        );
    }
}

#[test]
fn dispatch_config_commands_produce_correct_types() {
    // /setup -> OpenOverlay(Setup)
    let out = dispatch_command("setup", "");
    assert!(matches!(
        &out[0],
        CommandOutput::OpenOverlay(OverlayAction::Setup(None))
    ));

    // /settings -> OpenOverlay(Settings)
    let out = dispatch_command("settings", "");
    assert!(matches!(
        &out[0],
        CommandOutput::OpenOverlay(OverlayAction::Settings)
    ));

    // /init -> Info + BridgeRequest(InitConfig)
    let out = dispatch_command("init", "");
    assert!(matches!(&out[0], CommandOutput::Info(_)));
    assert!(matches!(
        &out[1],
        CommandOutput::BridgeRequest(BridgeAction::InitConfig { force: false })
    ));

    // /config with unknown key -> Error
    let out = dispatch_command("config", "key.path");
    assert!(matches!(&out[0], CommandOutput::Error(_)));

    // /factory-reset -> ConfirmAction(FactoryReset)
    let out = dispatch_command("factory-reset", "");
    assert!(matches!(
        &out[0],
        CommandOutput::ConfirmAction {
            action: BridgeAction::FactoryReset,
            ..
        }
    ));
}

#[test]
fn dispatch_files_commands_produce_output() {
    let file_cmds = [
        ("files", "src"),
        ("save", "output.txt"),
        ("validate", ""),
        ("discard", "temp.rs"),
        ("diff", "lib.rs"),
    ];

    for (cmd, args) in &file_cmds {
        let out = dispatch_command(cmd, args);
        assert!(
            !out.is_empty(),
            "Files command /{cmd} should produce output"
        );
    }
}

#[test]
fn dispatch_ai_commands_produce_output() {
    let out = dispatch_command("chain", "summarize");
    assert!(matches!(&out[0], CommandOutput::Info(_)));
    assert!(matches!(
        &out[1],
        CommandOutput::BridgeRequest(BridgeAction::RunChain { .. })
    ));

    let out = dispatch_command("prompts", "");
    assert!(matches!(&out[0], CommandOutput::Info(_)));
}

#[test]
fn dispatch_tools_commands_produce_output() {
    let out = dispatch_command("tools", "list");
    assert!(matches!(&out[0], CommandOutput::Info(_)));

    let out = dispatch_command("agents", "");
    assert!(matches!(&out[0], CommandOutput::Info(_)));

    let out = dispatch_command("skills", "");
    assert!(matches!(&out[0], CommandOutput::Info(_)));
}

#[test]
fn dispatch_meta_commands_produce_correct_types() {
    // /help -> Success
    let out = dispatch_command("help", "");
    assert!(matches!(&out[0], CommandOutput::Success(_)));

    // /clear -> Info("__clear__")
    let out = dispatch_command("clear", "");
    assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "__clear__"));

    // /exit -> Info("__exit__")
    let out = dispatch_command("exit", "");
    assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "__exit__"));

    // /shortcuts -> OpenOverlay(Shortcuts)
    let out = dispatch_command("shortcuts", "");
    assert!(matches!(
        &out[0],
        CommandOutput::OpenOverlay(OverlayAction::Shortcuts)
    ));

    // /compact -> Info + BridgeRequest(Compact)
    let out = dispatch_command("compact", "");
    assert!(matches!(&out[0], CommandOutput::Info(_)));
    assert!(matches!(
        &out[1],
        CommandOutput::BridgeRequest(BridgeAction::Compact)
    ));
}

#[test]
fn dispatch_unknown_command_returns_error() {
    let out = dispatch_command("nonexistent_command", "");
    assert!(matches!(&out[0], CommandOutput::Error(_)));
}

#[test]
fn dispatch_context_round_trip_parse_and_dispatch() {
    let inputs = [
        "/search hello world",
        "/help",
        "/verbose on",
        "/factory-reset",
        "/chain summarize",
    ];

    for input in &inputs {
        let (cmd, args) = parse_command_line(input).unwrap();
        let out = dispatch_command(&cmd, &args);
        assert!(
            !out.is_empty(),
            "Round-trip dispatch for '{input}' should produce output"
        );
    }
}

#[test]
fn dispatch_with_context_uses_state() {
    let ctx = DispatchContext {
        verbose: true,
        plan: true,
        total_tokens: 50_000,
        context_percent: 75,
        commands: all_commands(),
        cmd_ctx: CommandContext::default(),
    };

    // /verbose with no args should toggle from current (true -> off).
    let out = ctx.dispatch("verbose", "");
    assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("off")));

    // /context shows real values.
    let out = ctx.dispatch("context", "");
    assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("50.0k") && msg.contains("75%")));
}

// ═══════════════════════════════════════════════════════════════
// 4. History navigation -> up/down cycles through history
// ═══════════════════════════════════════════════════════════════

#[test]
fn history_full_cycle_up_down_restores_draft() {
    let mut app = App::new();

    // Submit three messages to build history.
    for msg in &["alpha", "beta", "gamma"] {
        app.input = input::insert(&app.input, msg);
        app = update(app, AppMessage::Submit);
    }
    assert_eq!(app.history, vec!["alpha", "beta", "gamma"]);

    // Start typing a new message (the "draft").
    app.input = input::insert(&app.input, "draft text");

    // Up -> gamma (most recent)
    app = update(app, AppMessage::HistoryUp);
    assert_eq!(app.input.value, "gamma");
    assert_eq!(app.history_index, Some(2));

    // Up -> beta
    app = update(app, AppMessage::HistoryUp);
    assert_eq!(app.input.value, "beta");
    assert_eq!(app.history_index, Some(1));

    // Up -> alpha
    app = update(app, AppMessage::HistoryUp);
    assert_eq!(app.input.value, "alpha");
    assert_eq!(app.history_index, Some(0));

    // Up again -> stays at alpha (clamped)
    app = update(app, AppMessage::HistoryUp);
    assert_eq!(app.input.value, "alpha");
    assert_eq!(app.history_index, Some(0));

    // Down -> beta
    app = update(app, AppMessage::HistoryDown);
    assert_eq!(app.input.value, "beta");

    // Down -> gamma
    app = update(app, AppMessage::HistoryDown);
    assert_eq!(app.input.value, "gamma");

    // Down -> restores draft
    app = update(app, AppMessage::HistoryDown);
    assert_eq!(app.input.value, "draft text");
    assert_eq!(app.history_index, None);
}

#[test]
fn history_up_on_empty_history_is_noop() {
    let mut app = App::new();
    app.input = input::insert(&app.input, "something");

    app = update(app, AppMessage::HistoryUp);
    assert_eq!(app.input.value, "something");
    assert_eq!(app.history_index, None);
}

#[test]
fn history_down_without_prior_up_is_noop() {
    let mut app = App::new();
    app.history = vec!["first".into()];
    app.input = input::insert(&app.input, "current");

    app = update(app, AppMessage::HistoryDown);
    assert_eq!(app.input.value, "current");
}

#[test]
fn history_deduplicates_consecutive_and_caps() {
    let mut app = App::new();

    // Submit the same message multiple times.
    for _ in 0..5 {
        app.input = input::insert(&app.input, "repeat");
        app = update(app, AppMessage::Submit);
    }

    // Should deduplicate consecutive entries.
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.history[0], "repeat");
}

// ═══════════════════════════════════════════════════════════════
// 5. Permission mode cycling
// ═══════════════════════════════════════════════════════════════

#[test]
fn permission_mode_cycles_through_all_modes() {
    let mut app = App::new();
    assert_eq!(app.permission_mode, "ask");

    app = update(app, AppMessage::ShiftTab);
    assert_eq!(app.permission_mode, "auto");

    app = update(app, AppMessage::ShiftTab);
    assert_eq!(app.permission_mode, "bypass");

    app = update(app, AppMessage::ShiftTab);
    assert_eq!(app.permission_mode, "ask");

    // Full additional cycle to verify stability.
    app = update(app, AppMessage::ShiftTab);
    assert_eq!(app.permission_mode, "auto");
}

#[test]
fn permission_mode_rendered_in_status_bar() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new();
    app = update(app, AppMessage::ShiftTab); // -> "auto"

    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(
        content.contains("auto"),
        "Status bar should show 'auto' permission mode"
    );
}

// ═══════════════════════════════════════════════════════════════
// 6. Non-interactive mode parsing
// ═══════════════════════════════════════════════════════════════

#[test]
fn non_interactive_prompt_parsed() {
    let args = vec![
        "simse".into(),
        "-p".into(),
        "explain closures".into(),
    ];
    let result = parse_cli_args(&args);
    assert_eq!(result.prompt.as_deref(), Some("explain closures"));
    assert_eq!(result.format, "text");
}

#[test]
fn non_interactive_json_format() {
    let args = vec![
        "simse".into(),
        "--prompt".into(),
        "list files".into(),
        "--format".into(),
        "json".into(),
    ];
    let result = parse_cli_args(&args);
    assert_eq!(result.prompt.as_deref(), Some("list files"));
    assert_eq!(result.format, "json");
}

#[test]
fn non_interactive_with_server_and_agent() {
    let args = vec![
        "simse".into(),
        "-p".into(),
        "test".into(),
        "--server".into(),
        "ollama".into(),
        "--agent".into(),
        "coder".into(),
        "-v".into(),
    ];
    let result = parse_cli_args(&args);
    assert_eq!(result.prompt.as_deref(), Some("test"));
    assert_eq!(result.server.as_deref(), Some("ollama"));
    assert_eq!(result.agent.as_deref(), Some("coder"));
    assert!(result.verbose);
}

#[test]
fn interactive_mode_when_no_prompt() {
    let args = vec![
        "simse".into(),
        "--continue".into(),
        "-v".into(),
    ];
    let result = parse_cli_args(&args);
    assert!(result.prompt.is_none());
    assert!(result.continue_session);
    assert!(result.verbose);
}

#[test]
fn help_flag_parsed() {
    let short = parse_cli_args(&["simse".into(), "-h".into()]);
    assert!(short.help);

    let long = parse_cli_args(&["simse".into(), "--help".into()]);
    assert!(long.help);
}

#[test]
fn resume_session_parsed() {
    let args = vec![
        "simse".into(),
        "--resume".into(),
        "sess-abc-123".into(),
    ];
    let result = parse_cli_args(&args);
    assert_eq!(result.resume.as_deref(), Some("sess-abc-123"));
}

// ═══════════════════════════════════════════════════════════════
// 7. Autocomplete activation and navigation
// ═══════════════════════════════════════════════════════════════

#[test]
fn autocomplete_activates_on_slash_prefix() {
    let cmds = all_commands();
    let mut state = CommandAutocompleteState::new();

    state.activate("/", &cmds);
    assert!(state.is_active());
    assert!(!state.visible_matches().is_empty());
}

#[test]
fn autocomplete_narrows_as_user_types() {
    let cmds = all_commands();
    let mut state = CommandAutocompleteState::new();

    state.activate("/", &cmds);
    let all_count = state.matches.len();

    state.update_matches("/hel", &cmds);
    let narrowed_count = state.matches.len();

    assert!(
        narrowed_count < all_count,
        "Typing more characters should narrow matches"
    );
    assert!(state.matches.iter().any(|m| m.name == "help"));
}

#[test]
fn autocomplete_navigate_and_accept() {
    let cmds = all_commands();
    let mut state = CommandAutocompleteState::new();

    state.activate("/hel", &cmds);
    assert!(state.is_active());

    // Move down and accept.
    let first_name = state.matches[0].name.clone();
    let result = state.accept();
    assert_eq!(result, Some(format!("/{first_name}")));
    assert!(!state.is_active());
}

#[test]
fn autocomplete_ghost_text_for_unique_match() {
    let cmds = all_commands();
    let mut state = CommandAutocompleteState::new();

    state.activate("/compac", &cmds);
    if state.matches.len() == 1 {
        assert_eq!(state.ghost_text(), Some("t".into()));
    }
}

#[test]
fn autocomplete_deactivates_on_non_slash_input() {
    let cmds = all_commands();
    let mut state = CommandAutocompleteState::new();

    state.activate("/h", &cmds);
    assert!(state.is_active());

    state.update_matches("hello", &cmds);
    assert!(!state.is_active());
}

#[test]
fn autocomplete_full_workflow_type_navigate_accept() {
    let cmds = all_commands();
    let mut state = CommandAutocompleteState::new();

    // 1. Activate.
    state.activate("/", &cmds);
    assert!(state.is_active());

    // 2. Narrow to commands starting with "se".
    state.update_matches("/se", &cmds);
    assert!(state.is_active());

    // 3. Navigate.
    let original_selected = state.selected;
    state.move_down();
    if state.matches.len() > 1 {
        assert_ne!(state.selected, original_selected);
    }

    // 4. Accept.
    let accepted = state.accept();
    assert!(accepted.is_some());
    assert!(accepted.unwrap().starts_with('/'));
    assert!(!state.is_active());
}

// ═══════════════════════════════════════════════════════════════
// 8. Settings explorer navigation
// ═══════════════════════════════════════════════════════════════

#[test]
fn settings_file_list_navigation() {
    let mut state = SettingsExplorerState::new();
    assert_eq!(state.level, SettingsLevel::FileList);
    assert_eq!(state.selected_file, 0);
    assert_eq!(state.selected_file_name(), "config.json");

    // Navigate down to "mcp.json" (index 2).
    state.move_down(CONFIG_FILES.len());
    state.move_down(CONFIG_FILES.len());
    assert_eq!(state.selected_file, 2);
    assert_eq!(state.selected_file_name(), "mcp.json");
    assert_eq!(state.selected_file_label(), "MCP Servers");
}

#[test]
fn settings_enter_field_list_and_navigate() {
    let mut state = SettingsExplorerState::new();

    // Enter a config file -> FieldList.
    state.enter("");
    assert_eq!(state.level, SettingsLevel::FieldList);
    assert_eq!(state.selected_field, 0);

    // Navigate fields.
    state.move_down(5);
    assert_eq!(state.selected_field, 1);
    state.move_down(5);
    assert_eq!(state.selected_field, 2);
    state.move_up();
    assert_eq!(state.selected_field, 1);
}

#[test]
fn settings_enter_editing_and_modify() {
    let mut state = SettingsExplorerState::new();

    state.enter(""); // -> FieldList
    state.enter("localhost"); // -> Editing

    assert_eq!(state.level, SettingsLevel::Editing);
    assert_eq!(state.edit_value, "localhost");

    // Modify the value.
    state.backspace(); // "localhos"
    state.backspace(); // "localho"
    state.type_char('a'); // "localhoa"
    assert_eq!(state.edit_value, "localhoa");
}

#[test]
fn settings_back_navigation_full_cycle() {
    let mut state = SettingsExplorerState::new();

    // FileList -> FieldList -> Editing
    state.enter("");
    state.enter("value");
    assert_eq!(state.level, SettingsLevel::Editing);

    // Editing -> FieldList
    let dismiss = state.back();
    assert!(!dismiss);
    assert_eq!(state.level, SettingsLevel::FieldList);
    assert!(state.edit_value.is_empty());

    // FieldList -> FileList
    let dismiss = state.back();
    assert!(!dismiss);
    assert_eq!(state.level, SettingsLevel::FileList);

    // FileList -> dismiss
    let dismiss = state.back();
    assert!(dismiss);
}

#[test]
fn settings_toggle_boolean() {
    let mut state = SettingsExplorerState::new();
    state.enter(""); // -> FieldList
    state.enter("true"); // -> Editing

    state.toggle();
    assert_eq!(state.edit_value, "false");

    state.toggle();
    assert_eq!(state.edit_value, "true");
}

#[test]
fn settings_render_all_levels_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let config_data = serde_json::json!({
        "host": "localhost",
        "port": 8080,
        "debug": true,
        "name": "test"
    });

    // FileList level.
    let mut state = SettingsExplorerState::new();
    terminal
        .draw(|frame| {
            let area = frame.area();
            simse_tui::overlays::settings::render_settings_explorer(
                frame,
                area,
                &state,
                &config_data,
            );
        })
        .unwrap();

    // FieldList level.
    state.enter("");
    terminal
        .draw(|frame| {
            let area = frame.area();
            simse_tui::overlays::settings::render_settings_explorer(
                frame,
                area,
                &state,
                &config_data,
            );
        })
        .unwrap();

    // Editing level.
    state.enter("localhost");
    terminal
        .draw(|frame| {
            let area = frame.area();
            simse_tui::overlays::settings::render_settings_explorer(
                frame,
                area,
                &state,
                &config_data,
            );
        })
        .unwrap();
}

// ═══════════════════════════════════════════════════════════════
// 9. Confirm dialog workflow
// ═══════════════════════════════════════════════════════════════

#[test]
fn confirm_dialog_starts_with_cancel_selected() {
    let state = ConfirmDialogState::new("Delete all data?");
    assert!(state.is_cancelled());
    assert!(!state.can_confirm());
    assert_eq!(state.selected, 0);
    assert_eq!(state.message, "Delete all data?");
}

#[test]
fn confirm_dialog_full_confirm_workflow() {
    let mut state = ConfirmDialogState::new("Reset config?");

    // Move to Yes.
    state.move_down();
    assert!(!state.is_cancelled());
    assert!(!state.can_confirm());

    // Type "yes".
    state.type_char('y');
    assert!(!state.can_confirm());
    state.type_char('e');
    assert!(!state.can_confirm());
    state.type_char('s');
    assert!(state.can_confirm());
}

#[test]
fn confirm_dialog_cancel_clears_input() {
    let mut state = ConfirmDialogState::new("Delete?");

    // Move to Yes and start typing.
    state.move_down();
    state.type_char('y');
    state.type_char('e');
    assert_eq!(state.yes_input, "ye");

    // Move back to No -> clears input.
    state.move_up();
    assert!(state.is_cancelled());
    assert!(state.yes_input.is_empty());

    // Typing on No is ignored.
    state.type_char('y');
    assert!(state.yes_input.is_empty());
}

#[test]
fn confirm_dialog_backspace_revokes_confirmation() {
    let mut state = ConfirmDialogState::new("Delete?");
    state.move_down();
    state.type_char('y');
    state.type_char('e');
    state.type_char('s');
    assert!(state.can_confirm());

    state.backspace();
    assert!(!state.can_confirm());
    assert_eq!(state.yes_input, "ye");
}

#[test]
fn confirm_dialog_case_insensitive() {
    let mut state = ConfirmDialogState::new("Reset?");
    state.move_down();
    state.type_char('Y');
    state.type_char('E');
    state.type_char('S');
    assert!(state.can_confirm());
}

#[test]
fn confirm_dialog_renders_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    // No selected.
    let state = ConfirmDialogState::new("Delete all global configs?");
    terminal
        .draw(|frame| {
            let area = frame.area();
            simse_tui::dialogs::confirm::render_confirm_dialog(frame, area, &state);
        })
        .unwrap();

    // Yes selected with typed "yes".
    let mut state2 = ConfirmDialogState::new("Reset everything?");
    state2.move_down();
    state2.type_char('y');
    state2.type_char('e');
    state2.type_char('s');
    terminal
        .draw(|frame| {
            let area = frame.area();
            simse_tui::dialogs::confirm::render_confirm_dialog(frame, area, &state2);
        })
        .unwrap();
}

// ═══════════════════════════════════════════════════════════════
// Cross-module integration: App + Commands + Rendering
// ═══════════════════════════════════════════════════════════════

#[test]
fn app_slash_help_renders_output() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new();
    app.input = input::insert(&app.input, "/help");
    app = update(app, AppMessage::Submit);

    // Output should have a CommandResult.
    assert!(app
        .output
        .iter()
        .any(|o| matches!(o, OutputItem::CommandResult { .. })));

    // Rendering should not panic.
    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();
}

#[test]
fn app_slash_verbose_toggles_and_renders() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new();
    assert!(!app.verbose);

    app.input = input::insert(&app.input, "/verbose");
    app = update(app, AppMessage::Submit);
    assert!(app.verbose);

    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(
        content.contains("verbose"),
        "Status bar should indicate verbose mode is on"
    );
}

#[test]
fn app_tool_call_lifecycle_renders() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new();

    // Start a tool call.
    let tc = ToolCallState {
        id: "tc-int-1".into(),
        name: "read_file".into(),
        args: r#"{"path": "test.rs"}"#.into(),
        status: ToolCallStatus::Active,
        started_at: 1000,
        duration_ms: None,
        summary: None,
        error: None,
        diff: None,
    };
    app = update(app, AppMessage::ToolCallStart(tc));
    assert_eq!(app.loop_status, LoopStatus::ToolExecuting);
    assert_eq!(app.active_tool_calls.len(), 1);

    // Render with active tool call.
    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();

    // Complete the tool call.
    app = update(
        app,
        AppMessage::ToolCallEnd {
            id: "tc-int-1".into(),
            status: ToolCallStatus::Completed,
            summary: Some("Read 42 lines".into()),
            error: None,
            duration_ms: Some(150),
            diff: None,
        },
    );
    assert!(app.active_tool_calls.is_empty());
    assert!(app
        .output
        .iter()
        .any(|o| matches!(o, OutputItem::ToolCall(..))));

    // Render with completed tool call in output.
    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();
}

#[test]
fn app_stream_lifecycle() {
    let mut app = App::new();

    // Start streaming.
    app = update(app, AppMessage::StreamStart);
    assert_eq!(app.loop_status, LoopStatus::Streaming);

    // Receive deltas.
    app = update(app, AppMessage::StreamDelta("Hello ".into()));
    app = update(app, AppMessage::StreamDelta("world!".into()));
    assert_eq!(app.stream_text, "Hello world!");

    // End stream.
    app = update(
        app,
        AppMessage::StreamEnd {
            text: "Hello world! Complete response.".into(),
        },
    );
    assert!(app.stream_text.is_empty());
    assert!(app.output.iter().any(|o| matches!(o, OutputItem::Message {
        role,
        text
    } if role == "assistant" && text.contains("Complete response"))));
}

#[test]
fn app_ctrl_c_timeout_flow() {
    let mut app = App::new();

    // First Ctrl+C -> pending.
    app = update(app, AppMessage::CtrlC);
    assert!(app.ctrl_c_pending);
    assert!(!app.should_quit);

    // Timeout resets.
    app = update(app, AppMessage::CtrlCTimeout);
    assert!(!app.ctrl_c_pending);

    // First Ctrl+C again -> pending.
    app = update(app, AppMessage::CtrlC);
    assert!(app.ctrl_c_pending);

    // Second Ctrl+C -> quit.
    app = update(app, AppMessage::CtrlC);
    assert!(app.should_quit);
}

#[test]
fn app_exit_bare_word() {
    let mut app = App::new();
    app.input = input::insert(&app.input, "exit");
    app = update(app, AppMessage::Submit);
    assert!(app.should_quit);
}

#[test]
fn app_quit_bare_word() {
    let mut app = App::new();
    app.input = input::insert(&app.input, "quit");
    app = update(app, AppMessage::Submit);
    assert!(app.should_quit);
}

#[test]
fn app_escape_interrupts_streaming() {
    let mut app = App::new();
    app.loop_status = LoopStatus::Streaming;

    app = update(app, AppMessage::Escape);
    assert_eq!(app.loop_status, LoopStatus::Idle);
    assert!(app.output.iter().any(|o| matches!(o, OutputItem::Info { text } if text.contains("Interrupt"))));
}

#[test]
fn app_clear_screen_restores_banner() {
    let mut app = App::new();

    // Submit something to hide the banner.
    app.input = input::insert(&app.input, "hello");
    app = update(app, AppMessage::Submit);
    assert!(!app.banner_visible);

    // Ctrl+L clears and restores banner.
    app = update(app, AppMessage::CtrlL);
    assert!(app.output.is_empty());
    assert!(app.banner_visible);
}

#[test]
fn app_permission_request_shows_overlay() {
    let mut app = App::new();
    let req = PermissionRequest {
        id: "perm-1".into(),
        tool_name: "bash".into(),
        args: serde_json::json!({"command": "ls -la"}),
        options: vec![
            PermissionOption {
                id: "allow_once".into(),
                label: "Allow once".into(),
            },
            PermissionOption {
                id: "deny".into(),
                label: "Deny".into(),
            },
        ],
    };

    app = update(app, AppMessage::PermissionRequested(req.clone()));
    assert_eq!(app.screen, Screen::Permission(req));

    // Responding dismisses.
    app = update(
        app,
        AppMessage::PermissionResponse {
            id: "perm-1".into(),
            option_id: "allow_once".into(),
        },
    );
    assert_eq!(app.screen, Screen::Chat);
}

#[test]
fn app_shortcuts_overlay_toggle() {
    let mut app = App::new();

    // '?' on empty input -> shortcuts.
    app = update(app, AppMessage::CharInput('?'));
    assert_eq!(app.screen, Screen::Shortcuts);

    // Any key dismisses shortcuts.
    app = update(app, AppMessage::CharInput('a'));
    assert_eq!(app.screen, Screen::Chat);
}

#[test]
fn app_token_usage_accumulates_and_renders() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new();

    app = update(
        app,
        AppMessage::TokenUsage {
            prompt: 1000,
            completion: 500,
        },
    );
    assert_eq!(app.total_tokens, 1500);

    app = update(
        app,
        AppMessage::TokenUsage {
            prompt: 2000,
            completion: 1000,
        },
    );
    assert_eq!(app.total_tokens, 4500);

    // Rendering with tokens should show token count in status bar.
    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(
        content.contains("4.5k"),
        "Status bar should show '4.5k' tokens"
    );
}

// ═══════════════════════════════════════════════════════════════
// Small terminal rendering (no panics)
// ═══════════════════════════════════════════════════════════════

#[test]
fn render_on_tiny_terminal_does_not_panic() {
    let backend = TestBackend::new(20, 5);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new();
    app.input = input::insert(&app.input, "test");
    app = update(app, AppMessage::Submit);

    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();
}

#[test]
fn render_with_many_output_items_does_not_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new();
    for i in 0..50 {
        app.output.push(OutputItem::Message {
            role: "user".into(),
            text: format!("Message {i}"),
        });
    }

    terminal
        .draw(|frame| {
            view(&app, frame);
        })
        .unwrap();
}
