use cosmic_text::Action;
use cosmic_text::BorrowedWithFontSystem;
use cosmic_text::Edit;
use cosmic_text::Editor;
use cosmic_text::Motion;
use cosmic_text::Selection;

use crate::TextInputFilter;
use crate::clipboard::ClipboardRead;
use crate::edit::apply_action;
use crate::edit::apply_motion;
use crate::edit::buffer_len;
use crate::edit::cursor_at_line_end;

/// Actions that can be recieved by a text input
#[derive(Debug)]
pub enum TextInputAction {
    /// Submit Text
    Submit,
    /// Send currently selected text to the clipboard
    Copy,
    /// Send currently selected text to the clipboard, then delete it
    Cut,
    /// Retrieve text from the clipboard and then queue
    Paste,
    /// Paste text from the clipboard
    PasteDeferred(ClipboardRead),
    /// A single edit action
    Edit(TextInputEdit),
}

/// An edit to perform on a [`TextInputBuffer`](crate::TextInputBuffer)
#[derive(Debug)]
pub enum TextInputEdit {
    /// Move the cursor with some motion
    Motion(Motion, bool),
    /// Escape, clears selection
    Escape,
    /// Insert character at cursor
    Insert(char, bool),
    /// Create new line
    Enter,
    /// Delete text behind cursor
    Backspace,
    /// Delete text in front of cursor
    Delete,
    // Indent text (typically Tab)
    Indent,
    // Unindent text (typically Shift+Tab)
    Unindent,
    /// Mouse click at specified position
    Click {
        x: i32,
        y: i32,
    },
    /// Mouse double click at specified position
    DoubleClick {
        x: i32,
        y: i32,
    },
    /// Mouse triple click at specified position
    TripleClick {
        x: i32,
        y: i32,
    },
    /// Mouse drag to specified position
    Drag {
        x: i32,
        y: i32,
    },
    /// Scroll specified number of lines
    Scroll {
        lines: i32,
    },
    Paste(String),
    Undo,
    Redo,
    SelectAll,
}

/// apply a single `TextInputEdit` to a text editor buffer
pub fn apply_text_input_edit(
    edit: TextInputEdit,
    editor: &mut BorrowedWithFontSystem<'_, Editor<'static>>,
    changes: &mut cosmic_undo_2::Commands<cosmic_text::Change>,
    max_chars: Option<usize>,
    filter_mode: Option<&TextInputFilter>,
) {
    editor.start_change();

    match edit {
        TextInputEdit::Motion(motion, with_select) => {
            apply_motion(editor, with_select, motion);
        }
        TextInputEdit::Escape => {
            editor.action(Action::Escape);
        }
        TextInputEdit::Insert(ch, overwrite) => {
            if editor.selection() != Selection::None {
                editor.action(Action::Insert(ch));
            } else if overwrite && !cursor_at_line_end(editor) {
                editor.action(Action::Delete);
                editor.action(Action::Insert(ch));
            } else if max_chars.is_none_or(|max_chars| editor.with_buffer(buffer_len) < max_chars) {
                editor.action(Action::Insert(ch));
            }
        }
        TextInputEdit::Backspace => {
            if editor.delete_selection() {
                editor.set_redraw(true);
            } else {
                editor.action(Action::Backspace);
            }
        }
        TextInputEdit::Delete => {
            if editor.delete_selection() {
                editor.set_redraw(true);
            } else {
                editor.action(Action::Delete);
            }
        }
        TextInputEdit::Indent => {
            editor.action(Action::Indent);
        }
        TextInputEdit::Unindent => {
            editor.action(Action::Unindent);
        }
        TextInputEdit::Click { x, y } => {
            editor.action(Action::Click { x, y });
        }
        TextInputEdit::DoubleClick { x, y } => {
            editor.action(Action::DoubleClick { x, y });
        }
        TextInputEdit::TripleClick { x, y } => {
            editor.action(Action::DoubleClick { x, y });
        }
        TextInputEdit::Drag { x, y } => {
            editor.action(Action::Drag { x, y });
        }
        TextInputEdit::Scroll { lines } => {
            editor.action(Action::Scroll { lines });
        }
        TextInputEdit::Paste(text) => {
            if max_chars.is_none_or(|max| editor.with_buffer(buffer_len) + text.len() <= max) {
                editor.insert_string(&text, None);
            }
        }
        TextInputEdit::Undo => {
            for action in changes.undo() {
                apply_action(editor, action);
                editor.set_redraw(true);
            }
        }
        TextInputEdit::Redo => {
            for action in changes.redo() {
                apply_action(editor, action);
                editor.set_redraw(true);
            }
        }
        TextInputEdit::SelectAll => {
            editor.action(Action::Motion(Motion::BufferStart));
            let cursor = editor.cursor();
            editor.set_selection(Selection::Normal(cursor));
            editor.action(Action::Motion(Motion::BufferEnd));
        }
        TextInputEdit::Enter => {
            editor.action(Action::Enter);
        }
    }

    let Some(mut change) = editor.finish_change() else {
        return;
    };

    if change.items.is_empty() {
        return;
    }

    if let Some(filter_mode) = filter_mode {
        let text = editor.with_buffer(crate::get_text);
        if !filter_mode.is_match(&text) {
            change.reverse();
            editor.apply_change(&change);
            return;
        }
    }

    changes.push(change);
    editor.set_redraw(true);
}
