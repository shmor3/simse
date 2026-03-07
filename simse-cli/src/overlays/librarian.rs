//! Librarian explorer: overlay for browsing, creating, editing, and deleting librarians.
//!
//! Two navigation modes:
//! 1. **List** — browse existing librarians, with a "+ New librarian..." entry at the bottom
//! 2. **Detail** — edit name, description, permissions, and topic preferences
//!
//! # Layout (List mode)
//!
//! ```text
//! +-- Librarian Explorer -------------------------+
//! |                                               |
//! |  > my-librarian     General purpose lib       |
//! |    code-reviewer    Reviews code changes      |
//! |    + New librarian...                         |
//! |                                               |
//! |  up/dn navigate  enter open  esc dismiss      |
//! +-----------------------------------------------+
//! ```
//!
//! # Layout (Detail mode)
//!
//! ```text
//! +-- my-librarian --------------------------------+
//! |                                                |
//! |  > name: my-librarian                          |
//! |    description: General purpose lib             |
//! |    permissions: add, delete, reorganize         |
//! |    topics: **, rust                             |
//! |                                                |
//! |    [delete]                                    |
//! |                                                |
//! |  up/dn navigate  enter edit  <- back           |
//! +------------------------------------------------+
//! ```
//!
//! Field validation:
//! - Names must be kebab-case (lowercase letters, numbers, hyphens).
//! - Permissions and topics are comma-separated arrays.
//!
//! Auto-save on field edit. Delete with confirmation.

use ratatui::{
	layout::Rect,
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, Clear, Paragraph, Wrap},
	Frame,
};

// -- Constants ----------------------------------------------------------------

/// Maximum width of the librarian explorer popup.
const MAX_POPUP_WIDTH: u16 = 60;

/// Minimum width of the librarian explorer popup.
const MIN_POPUP_WIDTH: u16 = 34;

/// Detail mode field labels.
const DETAIL_FIELDS: &[&str] = &["name", "description", "permissions", "topics"];

/// Number of navigable items in detail mode: 4 fields + 1 delete action.
const DETAIL_ITEM_COUNT: usize = 5;

/// Index of the delete action row in detail mode.
const DELETE_FIELD_INDEX: usize = 4;

// -- LibrarianMode ------------------------------------------------------------

/// Which mode the librarian explorer is currently in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibrarianMode {
	/// Browsing the list of librarians.
	List,
	/// Viewing/editing the details of a selected librarian.
	Detail,
}

// -- LibrarianEntry -----------------------------------------------------------

/// A single librarian entry with its metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarianEntry {
	/// Kebab-case name of the librarian.
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Permission labels (e.g. "add", "delete", "reorganize").
	pub permissions: Vec<String>,
	/// Topic glob patterns (e.g. "**", "rust", "web/**").
	pub topics: Vec<String>,
}

impl LibrarianEntry {
	/// Create a new entry with default values.
	pub fn default_new() -> Self {
		Self {
			name: String::new(),
			description: String::new(),
			permissions: vec![
				"add".to_string(),
				"delete".to_string(),
				"reorganize".to_string(),
			],
			topics: vec!["**".to_string()],
		}
	}
}

// -- LibrarianExplorerState ---------------------------------------------------

/// State for the librarian explorer overlay.
///
/// Tracks the current mode (List vs Detail), the list of librarian entries,
/// the currently selected index, and the in-progress edit buffer.
#[derive(Debug, Clone)]
pub struct LibrarianExplorerState {
	/// Current navigation mode.
	pub mode: LibrarianMode,
	/// All known librarian entries.
	pub librarians: Vec<LibrarianEntry>,
	/// Index of the selected item in the current mode's list.
	///
	/// In List mode: index into `librarians` (or `librarians.len()` for "+ New").
	/// In Detail mode: index of the selected field (0..DETAIL_ITEM_COUNT).
	pub selected: usize,
	/// Which field is currently being edited in Detail mode.
	/// `None` means browsing, `Some(i)` means editing field `i`.
	pub editing_field: Option<usize>,
	/// The current edit buffer contents.
	pub edit_value: String,
	/// Index of the librarian being viewed in Detail mode.
	/// Only meaningful when `mode == LibrarianMode::Detail`.
	viewing_librarian: usize,
}

impl LibrarianExplorerState {
	/// Create a new librarian explorer state in List mode.
	pub fn new(librarians: Vec<LibrarianEntry>) -> Self {
		Self {
			mode: LibrarianMode::List,
			librarians,
			selected: 0,
			editing_field: None,
			edit_value: String::new(),
			viewing_librarian: 0,
		}
	}

	/// Move selection up within the current mode.
	pub fn move_up(&mut self) {
		if self.selected > 0 {
			self.selected -= 1;
		}
	}

	/// Move selection down within the current mode.
	pub fn move_down(&mut self) {
		let max = match self.mode {
			// In List mode: librarians.len() items + 1 for "+ New librarian..."
			// so the last valid index is librarians.len().
			LibrarianMode::List => self.librarians.len(),
			// In Detail mode: DETAIL_ITEM_COUNT items (0-indexed).
			LibrarianMode::Detail => DETAIL_ITEM_COUNT - 1,
		};
		if self.selected < max {
			self.selected += 1;
		}
	}

	/// Press enter.
	///
	/// In List mode: if a librarian is selected, switch to Detail mode.
	/// If the "+ New librarian..." entry is selected, creates a new entry
	/// and switches to Detail mode.
	///
	/// In Detail mode: if not currently editing, start editing the selected
	/// field. If the selected field is the delete action, this is a no-op
	/// (the caller should handle deletion via `delete_current()`).
	pub fn enter(&mut self) {
		match self.mode {
			LibrarianMode::List => {
				if self.selected == self.librarians.len() {
					// "+ New librarian..." entry.
					self.add_new();
					self.viewing_librarian = self.librarians.len() - 1;
					self.mode = LibrarianMode::Detail;
					self.selected = 0;
					self.editing_field = None;
					self.edit_value.clear();
				} else if self.selected < self.librarians.len() {
					self.viewing_librarian = self.selected;
					self.mode = LibrarianMode::Detail;
					self.selected = 0;
					self.editing_field = None;
					self.edit_value.clear();
				}
			}
			LibrarianMode::Detail => {
				if self.editing_field.is_some() {
					// Already editing -- enter means "confirm".
					return;
				}
				if self.selected == DELETE_FIELD_INDEX {
					// Delete action -- caller should handle via delete_current().
					return;
				}
				// Start editing the selected field.
				if let Some(lib) = self.librarians.get(self.viewing_librarian) {
					let value = match self.selected {
						0 => lib.name.clone(),
						1 => lib.description.clone(),
						2 => lib.permissions.join(", "),
						3 => lib.topics.join(", "),
						_ => String::new(),
					};
					self.edit_value = value;
					self.editing_field = Some(self.selected);
				}
			}
		}
	}

	/// Go back one level, or signal dismissal.
	///
	/// If currently editing, cancels the edit. If in Detail mode, returns
	/// to List mode. If in List mode, signals that the overlay should be
	/// dismissed.
	///
	/// Returns `true` if the overlay should be dismissed.
	pub fn back(&mut self) -> bool {
		if self.editing_field.is_some() {
			self.cancel_edit();
			return false;
		}
		match self.mode {
			LibrarianMode::Detail => {
				self.mode = LibrarianMode::List;
				self.selected = self.viewing_librarian.min(self.librarians.len());
				false
			}
			LibrarianMode::List => true,
		}
	}

	/// Append a character to the edit buffer (only when editing a field).
	pub fn type_char(&mut self, c: char) {
		if self.editing_field.is_some() {
			self.edit_value.push(c);
		}
	}

	/// Delete the last character from the edit buffer (only when editing).
	pub fn backspace(&mut self) {
		if self.editing_field.is_some() {
			self.edit_value.pop();
		}
	}

	/// Confirm the current edit and apply it to the librarian entry.
	///
	/// Validates the field value:
	/// - **name**: must be valid kebab-case. If invalid, the edit is discarded.
	/// - **permissions** / **topics**: parsed as comma-separated arrays.
	/// - **description**: accepted as-is.
	///
	/// Returns `true` if the edit was successfully applied.
	pub fn confirm_edit(&mut self) -> bool {
		let field_idx = match self.editing_field {
			Some(idx) => idx,
			None => return false,
		};

		let lib_idx = self.viewing_librarian;
		let value = self.edit_value.clone();

		let applied = if let Some(lib) = self.librarians.get_mut(lib_idx) {
			match field_idx {
				0 => {
					// Name: validate kebab-case.
					let trimmed = value.trim().to_string();
					if Self::is_valid_name(&trimmed) {
						lib.name = trimmed;
						true
					} else {
						false
					}
				}
				1 => {
					// Description: accept as-is.
					lib.description = value.trim().to_string();
					true
				}
				2 => {
					// Permissions: comma-separated.
					lib.permissions = Self::parse_comma_separated(&value);
					true
				}
				3 => {
					// Topics: comma-separated.
					lib.topics = Self::parse_comma_separated(&value);
					true
				}
				_ => false,
			}
		} else {
			false
		};

		self.editing_field = None;
		self.edit_value.clear();
		applied
	}

	/// Cancel the current edit without applying changes.
	pub fn cancel_edit(&mut self) {
		self.editing_field = None;
		self.edit_value.clear();
	}

	/// Create a new librarian with default values and append it to the list.
	pub fn add_new(&mut self) {
		self.librarians.push(LibrarianEntry::default_new());
	}

	/// Remove the currently viewed librarian (in Detail mode).
	///
	/// After deletion, switches back to List mode with the selection clamped
	/// to the valid range.
	pub fn delete_current(&mut self) {
		let lib_idx = self.viewing_librarian;
		if lib_idx < self.librarians.len() {
			self.librarians.remove(lib_idx);
		}
		self.mode = LibrarianMode::List;
		self.editing_field = None;
		self.edit_value.clear();
		// Clamp selection.
		if self.librarians.is_empty() {
			self.selected = 0;
		} else {
			self.selected = lib_idx.min(self.librarians.len() - 1);
		}
	}

	/// Validate a name as kebab-case: lowercase letters, numbers, and hyphens.
	///
	/// Must match the pattern `^[a-z0-9]+(-[a-z0-9]+)*$`.
	pub fn is_valid_name(name: &str) -> bool {
		if name.is_empty() {
			return false;
		}

		let mut chars = name.chars().peekable();
		let mut segment_len: usize = 0;

		while let Some(c) = chars.next() {
			if c.is_ascii_lowercase() || c.is_ascii_digit() {
				segment_len += 1;
			} else if c == '-' {
				if segment_len == 0 {
					// Leading or consecutive hyphen.
					return false;
				}
				segment_len = 0;
				if chars.peek().is_none() {
					// Trailing hyphen.
					return false;
				}
			} else {
				return false;
			}
		}

		segment_len > 0
	}

	/// Return a reference to the librarian currently being viewed or selected.
	///
	/// In List mode: returns the librarian at the current selection index
	/// (if it is not the "+ New" entry).
	/// In Detail mode: returns the librarian being viewed/edited.
	pub fn selected_librarian(&self) -> Option<&LibrarianEntry> {
		let idx = match self.mode {
			LibrarianMode::List => self.selected,
			LibrarianMode::Detail => self.viewing_librarian,
		};
		self.librarians.get(idx)
	}

	/// Parse a comma-separated string into a vector of trimmed, non-empty strings.
	fn parse_comma_separated(s: &str) -> Vec<String> {
		s.split(',')
			.map(|part| part.trim().to_string())
			.filter(|part| !part.is_empty())
			.collect()
	}
}

// -- Rendering ----------------------------------------------------------------

/// Render the librarian explorer as a centered overlay popup.
pub fn render_librarian_explorer(
	frame: &mut Frame,
	area: Rect,
	state: &LibrarianExplorerState,
) {
	let mut lines: Vec<Line<'static>> = Vec::new();

	// Blank line for padding.
	lines.push(Line::from(""));

	match state.mode {
		LibrarianMode::List => {
			render_list(&mut lines, state);
		}
		LibrarianMode::Detail => {
			render_detail(&mut lines, state);
		}
	}

	// Blank separator.
	lines.push(Line::from(""));

	// Key hints.
	render_key_hints(&mut lines, state);

	// Trailing padding.
	lines.push(Line::from(""));

	// Build the title.
	let title = match state.mode {
		LibrarianMode::List => " Librarian Explorer ".to_string(),
		LibrarianMode::Detail => {
			let name = state
				.librarians
				.get(state.viewing_librarian)
				.map(|lib| lib.name.as_str())
				.unwrap_or("(new)");
			if name.is_empty() {
				" (new) ".to_string()
			} else {
				format!(" {} ", name)
			}
		}
	};

	// Calculate popup dimensions.
	let content_height = lines.len() as u16 + 2; // +2 for border
	let available_width = area.width.saturating_sub(4);
	let popup_width = MAX_POPUP_WIDTH
		.min(available_width)
		.max(MIN_POPUP_WIDTH)
		.min(area.width);
	let popup_height = content_height
		.min(area.height.saturating_sub(2))
		.min(area.height);

	// Center the popup.
	let popup_x = (area.width.saturating_sub(popup_width)) / 2;
	let popup_y = (area.height.saturating_sub(popup_height)) / 2;
	let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

	// Clear the area behind the popup, then render.
	frame.render_widget(Clear, popup_area);

	let border_color = match state.mode {
		LibrarianMode::List => Color::Cyan,
		LibrarianMode::Detail => {
			if state.editing_field.is_some() {
				Color::Yellow
			} else {
				Color::Blue
			}
		}
	};

	let popup = Paragraph::new(lines)
		.wrap(Wrap { trim: false })
		.block(
			Block::default()
				.borders(Borders::ALL)
				.border_style(Style::default().fg(border_color))
				.title(title),
		);

	frame.render_widget(popup, popup_area);
}

/// Render the list mode contents.
fn render_list(lines: &mut Vec<Line<'static>>, state: &LibrarianExplorerState) {
	for (i, lib) in state.librarians.iter().enumerate() {
		let selected = i == state.selected;
		let prefix = if selected { "  \u{276f} " } else { "    " };
		let color = if selected { Color::Cyan } else { Color::Reset };
		let mut style = Style::default().fg(color);
		if selected {
			style = style.add_modifier(Modifier::BOLD);
		}

		let desc_display = if lib.description.is_empty() {
			String::new()
		} else {
			format!("  {}", truncate_value(&lib.description, 40))
		};

		lines.push(Line::from(vec![
			Span::styled(format!("{prefix}{}", lib.name), style),
			Span::styled(desc_display, Style::default().fg(Color::DarkGray)),
		]));
	}

	// "+ New librarian..." entry.
	let new_selected = state.selected == state.librarians.len();
	let new_prefix = if new_selected { "  \u{276f} " } else { "    " };
	let new_color = if new_selected { Color::Cyan } else { Color::Reset };
	let mut new_style = Style::default().fg(new_color);
	if new_selected {
		new_style = new_style.add_modifier(Modifier::BOLD | Modifier::ITALIC);
	} else {
		new_style = new_style.add_modifier(Modifier::ITALIC);
	}

	lines.push(Line::from(Span::styled(
		format!("{new_prefix}+ New librarian..."),
		new_style,
	)));
}

/// Render the detail mode contents.
fn render_detail(lines: &mut Vec<Line<'static>>, state: &LibrarianExplorerState) {
	let lib = state
		.librarians
		.get(state.viewing_librarian)
		.cloned()
		.unwrap_or_else(LibrarianEntry::default_new);

	// Field rows.
	for (i, &label) in DETAIL_FIELDS.iter().enumerate() {
		let selected = i == state.selected;
		let is_editing = state.editing_field == Some(i);
		let prefix = if selected { "  \u{276f} " } else { "    " };
		let key_color = if selected { Color::Cyan } else { Color::White };
		let mut key_style = Style::default().fg(key_color);
		if selected {
			key_style = key_style.add_modifier(Modifier::BOLD);
		}

		let value_display = if is_editing {
			// Show the edit buffer with a cursor.
			let edit_text = if state.edit_value.is_empty() {
				"_".to_string()
			} else {
				state.edit_value.clone()
			};
			vec![
				Span::styled(format!("{prefix}{label}: "), key_style),
				Span::styled(edit_text, Style::default().fg(Color::White)),
				Span::styled(
					"\u{2588}",
					Style::default()
						.fg(Color::White)
						.add_modifier(Modifier::SLOW_BLINK),
				),
			]
		} else {
			let display = match i {
				0 => {
					if lib.name.is_empty() {
						"(empty)".to_string()
					} else {
						lib.name.clone()
					}
				}
				1 => {
					if lib.description.is_empty() {
						"(empty)".to_string()
					} else {
						lib.description.clone()
					}
				}
				2 => {
					if lib.permissions.is_empty() {
						"(none)".to_string()
					} else {
						lib.permissions.join(", ")
					}
				}
				3 => {
					if lib.topics.is_empty() {
						"(none)".to_string()
					} else {
						lib.topics.join(", ")
					}
				}
				_ => String::new(),
			};

			let is_dim = display.starts_with('(');
			let val_color = if is_dim {
				Color::DarkGray
			} else if selected {
				Color::Cyan
			} else {
				Color::Green
			};

			vec![
				Span::styled(format!("{prefix}{label}: "), key_style),
				Span::styled(display, Style::default().fg(val_color)),
			]
		};

		lines.push(Line::from(value_display));
	}

	// Blank separator before delete action.
	lines.push(Line::from(""));

	// Delete action row.
	let del_selected = state.selected == DELETE_FIELD_INDEX;
	let del_prefix = if del_selected { "  \u{276f} " } else { "    " };
	let del_color = if del_selected { Color::Cyan } else { Color::Red };
	let mut del_style = Style::default().fg(del_color);
	if del_selected {
		del_style = del_style.add_modifier(Modifier::BOLD);
	}

	lines.push(Line::from(Span::styled(
		format!("{del_prefix}\u{26a0} Delete librarian"),
		del_style,
	)));
}

/// Render key hints at the bottom of the overlay.
fn render_key_hints(lines: &mut Vec<Line<'static>>, state: &LibrarianExplorerState) {
	let dim = Style::default().fg(Color::DarkGray);
	let bold_dim = Style::default()
		.fg(Color::DarkGray)
		.add_modifier(Modifier::BOLD);

	let mut spans = Vec::new();
	spans.push(Span::raw("  "));

	match state.mode {
		LibrarianMode::List => {
			spans.push(Span::styled("\u{2191}\u{2193}", bold_dim));
			spans.push(Span::styled(" navigate  ", dim));
			spans.push(Span::styled("\u{21b5}", bold_dim));
			spans.push(Span::styled(" open  ", dim));
			spans.push(Span::styled("esc", bold_dim));
			spans.push(Span::styled(" dismiss", dim));
		}
		LibrarianMode::Detail => {
			if state.editing_field.is_some() {
				spans.push(Span::styled("\u{21b5}", bold_dim));
				spans.push(Span::styled(" save  ", dim));
				spans.push(Span::styled("esc", bold_dim));
				spans.push(Span::styled(" cancel", dim));
			} else {
				spans.push(Span::styled("\u{2191}\u{2193}", bold_dim));
				spans.push(Span::styled(" navigate  ", dim));
				spans.push(Span::styled("\u{21b5}", bold_dim));
				spans.push(Span::styled(" edit  ", dim));
				spans.push(Span::styled("\u{2190}", bold_dim));
				spans.push(Span::styled(" back  ", dim));
				spans.push(Span::styled("esc", bold_dim));
				spans.push(Span::styled(" dismiss", dim));
			}
		}
	}

	lines.push(Line::from(spans));
}

// -- Helpers ------------------------------------------------------------------

/// Truncate a display value to `max_len` characters, appending "..." if truncated.
fn truncate_value(s: &str, max_len: usize) -> String {
	if s.chars().count() <= max_len {
		s.to_string()
	} else {
		let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
		format!("{truncated}...")
	}
}

// -- Tests --------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -- helpers ---------------------------------------------------------------

	fn sample_librarians() -> Vec<LibrarianEntry> {
		vec![
			LibrarianEntry {
				name: "my-librarian".to_string(),
				description: "General purpose".to_string(),
				permissions: vec!["add".into(), "delete".into()],
				topics: vec!["**".into()],
			},
			LibrarianEntry {
				name: "code-reviewer".to_string(),
				description: "Reviews code changes".to_string(),
				permissions: vec!["add".into()],
				topics: vec!["rust".into(), "typescript".into()],
			},
		]
	}

	// -- LibrarianEntry::default_new ------------------------------------------

	#[test]
	fn default_new_has_expected_defaults() {
		let entry = LibrarianEntry::default_new();
		assert!(entry.name.is_empty());
		assert!(entry.description.is_empty());
		assert_eq!(entry.permissions, vec!["add", "delete", "reorganize"]);
		assert_eq!(entry.topics, vec!["**"]);
	}

	// -- LibrarianExplorerState::new ------------------------------------------

	#[test]
	fn new_starts_in_list_mode() {
		let state = LibrarianExplorerState::new(sample_librarians());
		assert_eq!(state.mode, LibrarianMode::List);
		assert_eq!(state.selected, 0);
		assert!(state.editing_field.is_none());
		assert!(state.edit_value.is_empty());
		assert_eq!(state.librarians.len(), 2);
	}

	#[test]
	fn new_with_empty_librarians() {
		let state = LibrarianExplorerState::new(vec![]);
		assert_eq!(state.mode, LibrarianMode::List);
		assert_eq!(state.selected, 0);
		assert!(state.librarians.is_empty());
	}

	// -- move_up / move_down --------------------------------------------------

	#[test]
	fn move_up_clamps_at_zero() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.move_up();
		assert_eq!(state.selected, 0);
	}

	#[test]
	fn move_down_in_list_mode() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.move_down();
		assert_eq!(state.selected, 1);
		state.move_down();
		assert_eq!(state.selected, 2); // "+ New librarian..."
	}

	#[test]
	fn move_down_clamps_in_list_mode() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		for _ in 0..20 {
			state.move_down();
		}
		// Max = librarians.len() = 2 (the "+ New" entry).
		assert_eq!(state.selected, 2);
	}

	#[test]
	fn move_up_in_list_mode() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.move_down();
		state.move_down();
		assert_eq!(state.selected, 2);
		state.move_up();
		assert_eq!(state.selected, 1);
		state.move_up();
		assert_eq!(state.selected, 0);
	}

	#[test]
	fn move_down_in_detail_mode() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail for first librarian
		assert_eq!(state.mode, LibrarianMode::Detail);
		state.move_down();
		assert_eq!(state.selected, 1); // description field
		state.move_down();
		assert_eq!(state.selected, 2); // permissions field
		state.move_down();
		assert_eq!(state.selected, 3); // topics field
		state.move_down();
		assert_eq!(state.selected, 4); // delete action
	}

	#[test]
	fn move_down_clamps_in_detail_mode() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		for _ in 0..20 {
			state.move_down();
		}
		assert_eq!(state.selected, DETAIL_ITEM_COUNT - 1);
	}

	#[test]
	fn move_up_in_detail_mode() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down();
		state.move_down();
		assert_eq!(state.selected, 2);
		state.move_up();
		assert_eq!(state.selected, 1);
	}

	// -- enter ----------------------------------------------------------------

	#[test]
	fn enter_from_list_goes_to_detail() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter();
		assert_eq!(state.mode, LibrarianMode::Detail);
		assert_eq!(state.selected, 0); // first field
		assert_eq!(state.viewing_librarian, 0);
	}

	#[test]
	fn enter_second_librarian_from_list() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.move_down();
		assert_eq!(state.selected, 1);
		state.enter();
		assert_eq!(state.mode, LibrarianMode::Detail);
		assert_eq!(state.viewing_librarian, 1);
		assert_eq!(state.selected, 0);
	}

	#[test]
	fn enter_new_librarian_from_list() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.move_down();
		state.move_down(); // -> "+ New librarian..."
		assert_eq!(state.selected, 2);
		state.enter();
		assert_eq!(state.mode, LibrarianMode::Detail);
		assert_eq!(state.librarians.len(), 3); // new one added
		assert_eq!(state.viewing_librarian, 2); // viewing the new one
	}

	#[test]
	fn enter_in_detail_starts_editing() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> start editing name field
		assert_eq!(state.editing_field, Some(0));
		assert_eq!(state.edit_value, "my-librarian");
	}

	#[test]
	fn enter_in_detail_editing_description() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down(); // -> description field
		state.enter(); // -> start editing
		assert_eq!(state.editing_field, Some(1));
		assert_eq!(state.edit_value, "General purpose");
	}

	#[test]
	fn enter_in_detail_editing_permissions() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down();
		state.move_down(); // -> permissions field
		state.enter();
		assert_eq!(state.editing_field, Some(2));
		assert_eq!(state.edit_value, "add, delete");
	}

	#[test]
	fn enter_in_detail_editing_topics() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down();
		state.move_down();
		state.move_down(); // -> topics field
		state.enter();
		assert_eq!(state.editing_field, Some(3));
		assert_eq!(state.edit_value, "**");
	}

	#[test]
	fn enter_on_delete_action_is_noop() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		for _ in 0..4 {
			state.move_down();
		}
		assert_eq!(state.selected, DELETE_FIELD_INDEX);
		state.enter(); // Should be a no-op.
		assert!(state.editing_field.is_none());
	}

	#[test]
	fn enter_while_editing_is_noop() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> editing name
		assert_eq!(state.editing_field, Some(0));
		let old_value = state.edit_value.clone();
		state.enter(); // Should be a no-op.
		assert_eq!(state.editing_field, Some(0));
		assert_eq!(state.edit_value, old_value);
	}

	// -- back -----------------------------------------------------------------

	#[test]
	fn back_from_detail_goes_to_list() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		let dismiss = state.back();
		assert!(!dismiss);
		assert_eq!(state.mode, LibrarianMode::List);
	}

	#[test]
	fn back_from_list_signals_dismiss() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		let dismiss = state.back();
		assert!(dismiss);
	}

	#[test]
	fn back_from_editing_cancels_edit() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> editing name
		assert!(state.editing_field.is_some());
		let dismiss = state.back();
		assert!(!dismiss);
		assert!(state.editing_field.is_none());
		assert!(state.edit_value.is_empty());
		assert_eq!(state.mode, LibrarianMode::Detail);
	}

	#[test]
	fn back_from_detail_restores_list_selection() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.move_down(); // select second librarian
		state.enter(); // -> Detail (viewing_librarian = 1)
		assert_eq!(state.viewing_librarian, 1);
		state.back();
		assert_eq!(state.mode, LibrarianMode::List);
		assert_eq!(state.selected, 1); // restored to second librarian
	}

	// -- type_char / backspace ------------------------------------------------

	#[test]
	fn type_char_appends_when_editing() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> editing name
		state.type_char('x');
		state.type_char('y');
		assert_eq!(state.edit_value, "my-librarianxy");
	}

	#[test]
	fn type_char_ignored_when_not_editing() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.type_char('x');
		assert!(state.edit_value.is_empty());
	}

	#[test]
	fn type_char_ignored_in_detail_browsing() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.type_char('x');
		assert!(state.edit_value.is_empty());
	}

	#[test]
	fn backspace_removes_last_char() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> editing name ("my-librarian")
		state.backspace();
		assert_eq!(state.edit_value, "my-libraria");
	}

	#[test]
	fn backspace_on_empty_is_noop() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down(); // -> description
		state.enter(); // -> editing description ("General purpose")
		// Clear the edit value.
		state.edit_value.clear();
		state.backspace();
		assert!(state.edit_value.is_empty());
	}

	#[test]
	fn backspace_ignored_when_not_editing() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.backspace();
		assert!(state.edit_value.is_empty());
	}

	// -- confirm_edit ---------------------------------------------------------

	#[test]
	fn confirm_edit_name_valid() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail (viewing librarian 0)
		state.enter(); // -> editing name
		state.edit_value = "new-name".to_string();
		let applied = state.confirm_edit();
		assert!(applied);
		assert_eq!(state.librarians[0].name, "new-name");
		assert!(state.editing_field.is_none());
	}

	#[test]
	fn confirm_edit_name_invalid_discards() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> editing name
		state.edit_value = "Invalid Name".to_string();
		let applied = state.confirm_edit();
		assert!(!applied);
		// Name unchanged.
		assert_eq!(state.librarians[0].name, "my-librarian");
		assert!(state.editing_field.is_none());
	}

	#[test]
	fn confirm_edit_description() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down(); // -> description
		state.enter(); // -> editing
		state.edit_value = "Updated description".to_string();
		let applied = state.confirm_edit();
		assert!(applied);
		assert_eq!(state.librarians[0].description, "Updated description");
	}

	#[test]
	fn confirm_edit_permissions_comma_separated() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down();
		state.move_down(); // -> permissions
		state.enter(); // -> editing
		state.edit_value = "read, write, admin".to_string();
		let applied = state.confirm_edit();
		assert!(applied);
		assert_eq!(
			state.librarians[0].permissions,
			vec!["read", "write", "admin"]
		);
	}

	#[test]
	fn confirm_edit_topics_comma_separated() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down();
		state.move_down();
		state.move_down(); // -> topics
		state.enter(); // -> editing
		state.edit_value = "rust, web/**, docs".to_string();
		let applied = state.confirm_edit();
		assert!(applied);
		assert_eq!(
			state.librarians[0].topics,
			vec!["rust", "web/**", "docs"]
		);
	}

	#[test]
	fn confirm_edit_no_editing_returns_false() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		let applied = state.confirm_edit();
		assert!(!applied);
	}

	#[test]
	fn confirm_edit_empty_name_is_invalid() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> editing name
		state.edit_value = "".to_string();
		let applied = state.confirm_edit();
		assert!(!applied);
		assert_eq!(state.librarians[0].name, "my-librarian");
	}

	#[test]
	fn confirm_edit_trims_whitespace() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down(); // -> description
		state.enter(); // -> editing
		state.edit_value = "  trimmed  ".to_string();
		let applied = state.confirm_edit();
		assert!(applied);
		assert_eq!(state.librarians[0].description, "trimmed");
	}

	#[test]
	fn confirm_edit_permissions_with_empty_parts() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down();
		state.move_down(); // -> permissions
		state.enter();
		state.edit_value = "add,, , delete, ".to_string();
		let applied = state.confirm_edit();
		assert!(applied);
		assert_eq!(state.librarians[0].permissions, vec!["add", "delete"]);
	}

	// -- cancel_edit ----------------------------------------------------------

	#[test]
	fn cancel_edit_discards_changes() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> editing name
		state.edit_value = "changed-name".to_string();
		state.cancel_edit();
		assert!(state.editing_field.is_none());
		assert!(state.edit_value.is_empty());
		// Name unchanged.
		assert_eq!(state.librarians[0].name, "my-librarian");
	}

	#[test]
	fn cancel_edit_when_not_editing_is_noop() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.cancel_edit();
		assert!(state.editing_field.is_none());
		assert!(state.edit_value.is_empty());
	}

	// -- add_new --------------------------------------------------------------

	#[test]
	fn add_new_appends_default_entry() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		assert_eq!(state.librarians.len(), 2);
		state.add_new();
		assert_eq!(state.librarians.len(), 3);
		let new = &state.librarians[2];
		assert!(new.name.is_empty());
		assert!(new.description.is_empty());
		assert_eq!(new.permissions.len(), 3);
		assert_eq!(new.topics, vec!["**"]);
	}

	#[test]
	fn add_new_on_empty_list() {
		let mut state = LibrarianExplorerState::new(vec![]);
		state.add_new();
		assert_eq!(state.librarians.len(), 1);
	}

	// -- delete_current -------------------------------------------------------

	#[test]
	fn delete_current_removes_librarian() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail (viewing librarian 0)
		state.delete_current();
		assert_eq!(state.librarians.len(), 1);
		assert_eq!(state.librarians[0].name, "code-reviewer");
		assert_eq!(state.mode, LibrarianMode::List);
	}

	#[test]
	fn delete_current_clamps_selection() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.move_down(); // select second librarian
		state.enter(); // -> Detail (viewing librarian 1)
		state.delete_current();
		assert_eq!(state.librarians.len(), 1);
		assert_eq!(state.selected, 0);
	}

	#[test]
	fn delete_current_on_single_librarian() {
		let libs = vec![LibrarianEntry {
			name: "only-one".to_string(),
			description: "The only librarian".to_string(),
			permissions: vec![],
			topics: vec![],
		}];
		let mut state = LibrarianExplorerState::new(libs);
		state.enter(); // -> Detail
		state.delete_current();
		assert!(state.librarians.is_empty());
		assert_eq!(state.selected, 0);
		assert_eq!(state.mode, LibrarianMode::List);
	}

	// -- is_valid_name --------------------------------------------------------

	#[test]
	fn valid_name_simple() {
		assert!(LibrarianExplorerState::is_valid_name("hello"));
	}

	#[test]
	fn valid_name_with_hyphens() {
		assert!(LibrarianExplorerState::is_valid_name("my-librarian"));
	}

	#[test]
	fn valid_name_with_numbers() {
		assert!(LibrarianExplorerState::is_valid_name("lib-v2"));
	}

	#[test]
	fn valid_name_single_char() {
		assert!(LibrarianExplorerState::is_valid_name("a"));
	}

	#[test]
	fn valid_name_single_digit() {
		assert!(LibrarianExplorerState::is_valid_name("0"));
	}

	#[test]
	fn valid_name_multi_segment() {
		assert!(LibrarianExplorerState::is_valid_name("a-b-c-d"));
	}

	#[test]
	fn invalid_name_empty() {
		assert!(!LibrarianExplorerState::is_valid_name(""));
	}

	#[test]
	fn invalid_name_uppercase() {
		assert!(!LibrarianExplorerState::is_valid_name("MyLib"));
	}

	#[test]
	fn invalid_name_spaces() {
		assert!(!LibrarianExplorerState::is_valid_name("my lib"));
	}

	#[test]
	fn invalid_name_leading_hyphen() {
		assert!(!LibrarianExplorerState::is_valid_name("-leading"));
	}

	#[test]
	fn invalid_name_trailing_hyphen() {
		assert!(!LibrarianExplorerState::is_valid_name("trailing-"));
	}

	#[test]
	fn invalid_name_double_hyphen() {
		assert!(!LibrarianExplorerState::is_valid_name("double--hyphen"));
	}

	#[test]
	fn invalid_name_underscore() {
		assert!(!LibrarianExplorerState::is_valid_name("under_score"));
	}

	#[test]
	fn invalid_name_special_chars() {
		assert!(!LibrarianExplorerState::is_valid_name("lib@name"));
	}

	#[test]
	fn invalid_name_just_hyphen() {
		assert!(!LibrarianExplorerState::is_valid_name("-"));
	}

	// -- selected_librarian ---------------------------------------------------

	#[test]
	fn selected_librarian_in_list_mode() {
		let state = LibrarianExplorerState::new(sample_librarians());
		let lib = state.selected_librarian().unwrap();
		assert_eq!(lib.name, "my-librarian");
	}

	#[test]
	fn selected_librarian_in_detail_mode() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.move_down(); // second librarian
		state.enter(); // -> Detail
		let lib = state.selected_librarian().unwrap();
		assert_eq!(lib.name, "code-reviewer");
	}

	#[test]
	fn selected_librarian_on_new_entry_returns_none() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.move_down();
		state.move_down(); // -> "+ New librarian..."
		assert!(state.selected_librarian().is_none());
	}

	#[test]
	fn selected_librarian_empty_list() {
		let state = LibrarianExplorerState::new(vec![]);
		assert!(state.selected_librarian().is_none());
	}

	// -- Full workflow tests --------------------------------------------------

	#[test]
	fn full_workflow_create_and_edit() {
		let mut state = LibrarianExplorerState::new(vec![]);
		assert_eq!(state.mode, LibrarianMode::List);

		// Navigate to "+ New librarian..." (only entry).
		assert_eq!(state.selected, 0); // Already pointing at "+ New"
		state.enter();
		assert_eq!(state.mode, LibrarianMode::Detail);
		assert_eq!(state.librarians.len(), 1);

		// Edit name.
		state.enter(); // editing name
		assert_eq!(state.editing_field, Some(0));
		state.edit_value = "my-new-lib".to_string();
		let applied = state.confirm_edit();
		assert!(applied);
		assert_eq!(state.librarians[0].name, "my-new-lib");

		// Edit description.
		state.move_down();
		state.enter();
		state.edit_value = "A brand new librarian".to_string();
		state.confirm_edit();
		assert_eq!(state.librarians[0].description, "A brand new librarian");

		// Go back to list.
		let dismiss = state.back();
		assert!(!dismiss);
		assert_eq!(state.mode, LibrarianMode::List);

		// Dismiss.
		let dismiss = state.back();
		assert!(dismiss);
	}

	#[test]
	fn full_workflow_edit_existing() {
		let mut state = LibrarianExplorerState::new(sample_librarians());

		// Navigate to second librarian.
		state.move_down();
		state.enter(); // -> Detail for code-reviewer.
		assert_eq!(state.viewing_librarian, 1);

		// Edit topics.
		state.move_down();
		state.move_down();
		state.move_down(); // -> topics
		state.enter();
		assert_eq!(state.edit_value, "rust, typescript");
		state.edit_value = "rust, go, python".to_string();
		state.confirm_edit();
		assert_eq!(
			state.librarians[1].topics,
			vec!["rust", "go", "python"]
		);
	}

	#[test]
	fn full_workflow_delete() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		assert_eq!(state.librarians.len(), 2);

		// Enter detail for first librarian.
		state.enter();
		assert_eq!(state.viewing_librarian, 0);

		// Delete.
		state.delete_current();
		assert_eq!(state.librarians.len(), 1);
		assert_eq!(state.librarians[0].name, "code-reviewer");
		assert_eq!(state.mode, LibrarianMode::List);
	}

	#[test]
	fn full_workflow_cancel_edit() {
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> editing name
		state.edit_value = "changed".to_string();

		// Use back() to cancel.
		let dismiss = state.back();
		assert!(!dismiss);
		assert!(state.editing_field.is_none());
		assert_eq!(state.librarians[0].name, "my-librarian");
	}

	// -- Render smoke tests ---------------------------------------------------

	#[test]
	fn render_list_mode_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = LibrarianExplorerState::new(sample_librarians());

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_librarian_explorer(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_detail_mode_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_librarian_explorer(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_editing_mode_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.enter(); // -> editing name

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_librarian_explorer(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_empty_list_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = LibrarianExplorerState::new(vec![]);

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_librarian_explorer(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_small_terminal_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(30, 10);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = LibrarianExplorerState::new(sample_librarians());

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_librarian_explorer(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_detail_delete_selected_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		for _ in 0..4 {
			state.move_down();
		}
		assert_eq!(state.selected, DELETE_FIELD_INDEX);

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_librarian_explorer(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_new_librarian_detail_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = LibrarianExplorerState::new(vec![]);
		state.enter(); // -> creates new librarian and enters Detail

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_librarian_explorer(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_editing_with_empty_value_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = LibrarianExplorerState::new(sample_librarians());
		state.enter(); // -> Detail
		state.move_down(); // -> description
		state.enter(); // -> editing description
		state.edit_value.clear();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_librarian_explorer(frame, area, &state);
			})
			.unwrap();
	}

	// -- parse_comma_separated ------------------------------------------------

	#[test]
	fn parse_comma_separated_basic() {
		let result = LibrarianExplorerState::parse_comma_separated("a, b, c");
		assert_eq!(result, vec!["a", "b", "c"]);
	}

	#[test]
	fn parse_comma_separated_with_empty() {
		let result = LibrarianExplorerState::parse_comma_separated("a,, ,b");
		assert_eq!(result, vec!["a", "b"]);
	}

	#[test]
	fn parse_comma_separated_empty_string() {
		let result = LibrarianExplorerState::parse_comma_separated("");
		assert!(result.is_empty());
	}

	#[test]
	fn parse_comma_separated_whitespace_only() {
		let result = LibrarianExplorerState::parse_comma_separated("  ,  ,  ");
		assert!(result.is_empty());
	}

	#[test]
	fn parse_comma_separated_single() {
		let result = LibrarianExplorerState::parse_comma_separated("single");
		assert_eq!(result, vec!["single"]);
	}

	// -- truncate_value -------------------------------------------------------

	#[test]
	fn truncate_value_short() {
		assert_eq!(truncate_value("hello", 10), "hello");
	}

	#[test]
	fn truncate_value_exact() {
		assert_eq!(truncate_value("hello", 5), "hello");
	}

	#[test]
	fn truncate_value_long() {
		let result = truncate_value("hello world!", 8);
		assert_eq!(result, "hello...");
	}
}
