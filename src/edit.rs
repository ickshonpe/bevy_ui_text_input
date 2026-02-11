use crate::SubmitText;
use crate::TextInputBuffer;
use crate::TextInputFilter;
use crate::TextInputGlobalState;
use crate::TextInputImeState;
use crate::TextInputMode;
use crate::TextInputNode;
use crate::TextInputQueue;
use crate::TextInputStyle;
use crate::actions::TextInputAction;
use crate::actions::TextInputEdit;
use crate::actions::apply_text_input_edit;
use crate::clipboard::Clipboard;
use crate::text_input_pipeline::TextInputPipeline;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageReader;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::Commands;
use bevy::ecs::system::Query;
use bevy::ecs::system::Res;
use bevy::ecs::system::ResMut;
use bevy::input::ButtonState;
use bevy::input::keyboard::Key;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::mouse::MouseWheel;
use bevy::input_focus::FocusedInput;
use bevy::input_focus::InputFocus;
use bevy::math::Rect;
use bevy::math::Vec2;
use bevy::picking::events::Click;
use bevy::picking::events::Drag;
use bevy::picking::events::Move;
use bevy::picking::events::Pointer;
use bevy::picking::events::Press;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerButton;
use bevy::time::Time;
use bevy::ui::ComputedNode;
use bevy::ui::UiGlobalTransform;
use bevy::window::{Ime, Window};
use cosmic_text::Action;
use cosmic_text::BorrowedWithFontSystem;
use cosmic_text::Change;
use cosmic_text::Edit;
use cosmic_text::Editor;
use cosmic_text::Motion;
use cosmic_text::Selection;

pub fn apply_action<'a>(
    editor: &mut BorrowedWithFontSystem<Editor<'a>>,
    action: cosmic_undo_2::Action<&Change>,
) {
    match action {
        cosmic_undo_2::Action::Do(change) => {
            editor.apply_change(change);
        }
        cosmic_undo_2::Action::Undo(change) => {
            let mut reversed = change.clone();
            reversed.reverse();
            editor.apply_change(&reversed);
        }
    }
}

pub fn apply_motion<'a>(
    editor: &mut BorrowedWithFontSystem<Editor<'a>>,
    shift_pressed: bool,
    motion: Motion,
) {
    if shift_pressed {
        if editor.selection() == Selection::None {
            let cursor = editor.cursor();
            editor.set_selection(Selection::Normal(cursor));
        }
    } else {
        editor.action(Action::Escape);
    }
    editor.action(Action::Motion(motion));
}

pub fn buffer_len(buffer: &cosmic_text::Buffer) -> usize {
    buffer
        .lines
        .iter()
        .map(|line| line.text().chars().count())
        .sum()
}

pub fn cursor_at_line_end(editor: &mut BorrowedWithFontSystem<Editor<'_>>) -> bool {
    let cursor = editor.cursor();
    editor.with_buffer(|buffer| {
        buffer
            .lines
            .get(cursor.line)
            .map(|line| cursor.index == line.text().len())
            .unwrap_or(false)
    })
}

pub(crate) fn is_buffer_empty(buffer: &cosmic_text::Buffer) -> bool {
    buffer.lines.is_empty() || (buffer.lines.len() == 1 && buffer.lines[0].text().is_empty())
}

pub(crate) fn on_drag_text_input(
    trigger: On<Pointer<Drag>>,
    mut node_query: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &mut TextInputBuffer,
        &TextInputNode,
    )>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    input_focus: Res<InputFocus>,
) {
    if trigger.button != PointerButton::Primary {
        return;
    }

    if input_focus
        .0
        .is_none_or(|input_focus_entity| input_focus_entity != trigger.entity)
    {
        return;
    }

    let Ok((node, transform, mut buffer, input)) = node_query.get_mut(trigger.entity) else {
        return;
    };

    if !input.is_enabled || !input.focus_on_pointer_down {
        return;
    }

    let rect = Rect::from_center_size(transform.translation, node.size());

    let position =
        trigger.pointer_location.position * node.inverse_scale_factor().recip() - rect.min;

    let mut editor = buffer
        .editor
        .borrow_with(&mut text_input_pipeline.font_system);

    let scroll = editor.with_buffer(|buffer| buffer.scroll());

    editor.action(Action::Drag {
        x: position.x as i32 + scroll.horizontal as i32,
        y: position.y as i32,
    });
}

pub(crate) fn on_text_input_pressed(
    trigger: On<Pointer<Press>>,
    mut node_query: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &mut TextInputBuffer,
        &TextInputNode,
    )>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    mut input_focus: ResMut<InputFocus>,
) {
    if trigger.button != PointerButton::Primary {
        return;
    }

    let Ok((node, transform, mut buffer, input)) = node_query.get_mut(trigger.entity) else {
        return;
    };

    if !input.is_enabled || !input.focus_on_pointer_down {
        return;
    }

    if input_focus
        .get()
        .is_none_or(|active_input| active_input != trigger.entity)
    {
        input_focus.set(trigger.entity);
    }

    let rect = Rect::from_center_size(transform.translation, node.size());

    let position =
        trigger.pointer_location.position * node.inverse_scale_factor().recip() - rect.min;

    let mut editor = buffer
        .editor
        .borrow_with(&mut text_input_pipeline.font_system);

    let scroll = editor.with_buffer(|buffer| buffer.scroll());

    editor.action(Action::Click {
        x: position.x as i32 + scroll.horizontal as i32,
        y: position.y as i32,
    });
}

/// Updates the scroll position of scrollable nodes in response to mouse input
pub fn mouse_wheel_scroll(
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    mut node_query: Query<(&mut TextInputBuffer, &TextInputNode, &mut TextInputQueue)>,
) {
    for mouse_wheel_event in mouse_wheel_events.read() {
        for (_, pointer_map) in hover_map.iter() {
            for (entity, _) in pointer_map.iter() {
                let Ok((mut buffer, input, mut queue)) = node_query.get_mut(*entity) else {
                    continue;
                };

                if !matches!(input.mode, TextInputMode::MultiLine { .. }) {
                    continue;
                }

                match mouse_wheel_event.unit {
                    MouseScrollUnit::Line => {
                        queue.add(TextInputAction::Edit(TextInputEdit::Scroll {
                            lines: -mouse_wheel_event.y as i32,
                        }));
                    }
                    MouseScrollUnit::Pixel => {
                        buffer.editor.with_buffer_mut(|buffer| {
                            let mut scroll = buffer.scroll();
                            scroll.vertical -= mouse_wheel_event.y;
                            buffer.set_scroll(scroll);
                        });
                    }
                };
            }
        }
    }
}

const MULTI_CLICK_PERIOD: f32 = 0.5; // seconds

#[derive(Component)]
pub struct MultiClickData {
    last_click_time: f32,
    click_count: usize,
}

pub fn on_multi_click_set_selection(
    click: On<Pointer<Click>>,
    time: Res<Time>,
    mut text_input_nodes: Query<(
        &TextInputNode,
        &mut TextInputQueue,
        &mut TextInputBuffer,
        &UiGlobalTransform,
        &ComputedNode,
    )>,
    mut multi_click_datas: Query<&mut MultiClickData>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    mut commands: Commands,
) {
    if click.button != PointerButton::Primary {
        return;
    }

    let entity = click.entity;

    let Ok((input, mut queue, mut buffer, transform, node)) = text_input_nodes.get_mut(entity)
    else {
        return;
    };

    if !input.is_enabled || !input.focus_on_pointer_down {
        return;
    }

    let now = time.elapsed_secs();
    if let Ok(mut multi_click_data) = multi_click_datas.get_mut(entity)
        && now - multi_click_data.last_click_time
            <= MULTI_CLICK_PERIOD * multi_click_data.click_count as f32
    {
        let rect = Rect::from_center_size(transform.translation, node.size());

        let position =
            click.pointer_location.position * node.inverse_scale_factor().recip() - rect.min;
        let mut editor = buffer
            .editor
            .borrow_with(&mut text_input_pipeline.font_system);
        let scroll = editor.with_buffer(|buffer| buffer.scroll());
        match multi_click_data.click_count {
            1 => {
                multi_click_data.click_count += 1;
                multi_click_data.last_click_time = now;

                queue.add(TextInputAction::Edit(TextInputEdit::DoubleClick {
                    x: position.x as i32 + scroll.horizontal as i32,
                    y: position.y as i32,
                }));
                return;
            }
            2 => {
                editor.action(Action::Motion(Motion::ParagraphStart));
                let cursor = editor.cursor();
                editor.set_selection(Selection::Normal(cursor));
                editor.action(Action::Motion(Motion::ParagraphEnd));
                if let Ok(mut entity) = commands.get_entity(entity) {
                    entity.try_remove::<MultiClickData>();
                }
                return;
            }
            _ => (),
        }
    }
    if let Ok(mut entity) = commands.get_entity(entity) {
        entity.try_insert(MultiClickData {
            last_click_time: now,
            click_count: 1,
        });
    }
}

pub fn on_move_clear_multi_click(move_: On<Pointer<Move>>, mut commands: Commands) {
    if let Ok(mut entity) = commands.get_entity(move_.entity) {
        entity.try_remove::<MultiClickData>();
    }
}

pub fn queue_text_input_action(
    input_mode: &TextInputMode,
    shift_pressed: &mut bool,
    overwrite_mode: &mut bool,
    command_pressed: &mut bool,
    keyboard_input: &KeyboardInput,
    mut queue: impl FnMut(TextInputAction),
) {
    match keyboard_input.logical_key {
        Key::Shift => {
            *shift_pressed = keyboard_input.state == ButtonState::Pressed;
            return;
        }
        Key::Control => {
            *command_pressed = keyboard_input.state == ButtonState::Pressed;
            return;
        }
        #[cfg(target_os = "macos")]
        Key::Super => {
            *command_pressed = keyboard_input.state == ButtonState::Pressed;
            return;
        }
        _ => {}
    };

    if keyboard_input.state.is_pressed() {
        if *command_pressed {
            match &keyboard_input.logical_key {
                Key::Character(str) => {
                    if let Some(char) = str.chars().next() {
                        // convert to lowercase so that the commands work with capslock on
                        match (char.to_ascii_lowercase(), *shift_pressed) {
                            ('c', false) => {
                                // copy
                                queue(TextInputAction::Copy);
                            }
                            ('x', false) => {
                                // cut
                                queue(TextInputAction::Cut);
                            }
                            ('v', false) => {
                                // paste
                                queue(TextInputAction::Paste);
                            }
                            ('z', false) => {
                                queue(TextInputAction::Edit(TextInputEdit::Undo));
                            }
                            #[cfg(target_os = "macos")]
                            ('z', true) => {
                                queue(TextInputAction::Edit(TextInputEdit::Redo));
                            }
                            ('y', false) => {
                                queue(TextInputAction::Edit(TextInputEdit::Redo));
                            }
                            ('a', false) => {
                                // select all
                                queue(TextInputAction::Edit(TextInputEdit::SelectAll));
                            }
                            _ => {
                                // not recognised, ignore
                            }
                        }
                    }
                }
                Key::ArrowLeft => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::PreviousWord,
                        *shift_pressed,
                    )));
                }
                Key::ArrowRight => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::NextWord,
                        *shift_pressed,
                    )));
                }
                Key::ArrowUp => {
                    if matches!(input_mode, TextInputMode::MultiLine { .. }) {
                        queue(TextInputAction::Edit(TextInputEdit::Scroll { lines: -1 }));
                    }
                }
                Key::ArrowDown => {
                    if matches!(input_mode, TextInputMode::MultiLine { .. }) {
                        queue(TextInputAction::Edit(TextInputEdit::Scroll { lines: 1 }));
                    }
                }
                Key::Home => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::BufferStart,
                        *shift_pressed,
                    )));
                }
                Key::End => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::BufferEnd,
                        *shift_pressed,
                    )));
                }
                _ => {
                    // not recognised, ignore
                }
            }
        } else {
            match &keyboard_input.logical_key {
                Key::Character(_) | Key::Space => {
                    let str = if let Key::Character(str) = &keyboard_input.logical_key {
                        str.chars()
                    } else {
                        " ".chars()
                    };
                    for char in str {
                        queue(TextInputAction::Edit(TextInputEdit::Insert(
                            char,
                            *overwrite_mode,
                        )));
                    }
                }
                Key::Enter => match (*shift_pressed, input_mode) {
                    (false, TextInputMode::MultiLine { .. }) => {
                        queue(TextInputAction::Edit(TextInputEdit::Enter));
                    }
                    _ => {
                        queue(TextInputAction::Submit);
                    }
                },
                Key::Backspace => {
                    queue(TextInputAction::Edit(TextInputEdit::Backspace));
                }
                Key::Delete => {
                    if *shift_pressed {
                        queue(TextInputAction::Cut);
                    } else {
                        queue(TextInputAction::Edit(TextInputEdit::Delete));
                    }
                }
                Key::PageUp => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::PageUp,
                        *shift_pressed,
                    )));
                }
                Key::PageDown => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::PageDown,
                        *shift_pressed,
                    )));
                }
                Key::ArrowLeft => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::Left,
                        *shift_pressed,
                    )));
                }
                Key::ArrowRight => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::Right,
                        *shift_pressed,
                    )));
                }
                Key::ArrowUp => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::Up,
                        *shift_pressed,
                    )));
                }
                Key::ArrowDown => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::Down,
                        *shift_pressed,
                    )));
                }
                Key::Home => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::Home,
                        *shift_pressed,
                    )));
                }
                Key::End => {
                    queue(TextInputAction::Edit(TextInputEdit::Motion(
                        Motion::End,
                        *shift_pressed,
                    )));
                }
                Key::Escape => {
                    queue(TextInputAction::Edit(TextInputEdit::Escape));
                }
                Key::Tab => {
                    if matches!(input_mode, TextInputMode::MultiLine { .. }) {
                        if *shift_pressed {
                            queue(TextInputAction::Edit(TextInputEdit::Unindent));
                        } else {
                            queue(TextInputAction::Edit(TextInputEdit::Indent));
                        }
                    }
                }
                Key::Insert => {
                    if !*shift_pressed {
                        *overwrite_mode = !*overwrite_mode;
                    }
                }
                _ => {}
            }
        }
    }
}

/// updates the cursor blink time for text inputs
pub fn cursor_blink_system(
    mut query: Query<(&mut TextInputBuffer, &TextInputStyle, &TextInputQueue)>,
    time: Res<Time>,
) {
    for (mut buffer, style, queue) in query.iter_mut() {
        buffer.cursor_blink_time = if queue.is_empty() {
            (buffer.cursor_blink_time + time.delta_secs()).rem_euclid(style.blink_interval * 2.)
        } else {
            0.
        };
    }
}

pub fn process_text_input_queues(
    mut query: Query<(
        Entity,
        &TextInputNode,
        &mut TextInputBuffer,
        &mut TextInputQueue,
        &mut TextInputImeState,
        Option<&TextInputFilter>,
    )>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    mut submit_writer: MessageWriter<SubmitText>,
    mut clipboard: ResMut<Clipboard>,
) {
    let font_system = &mut text_input_pipeline.font_system;

    for (entity, node, mut buffer, mut actions_queue, mut ime_state, maybe_filter) in
        query.iter_mut()
    {
        let TextInputBuffer {
            editor, changes, ..
        } = &mut *buffer;
        let mut editor = editor.borrow_with(font_system);
        while let Some(action) = actions_queue.next() {
            match action {
                TextInputAction::Submit => {
                    let text = editor.with_buffer(crate::get_text);
                    submit_writer.write(SubmitText { entity, text });
                    if node.clear_on_submit {
                        actions_queue.add_front(TextInputAction::Edit(TextInputEdit::Delete));
                        actions_queue.add_front(TextInputAction::Edit(TextInputEdit::SelectAll));
                    }
                }
                TextInputAction::Cut => {
                    if let Some(text) = editor.copy_selection() {
                        let _ = clipboard.set_text(text);
                        apply_text_input_edit(
                            TextInputEdit::Delete,
                            &mut editor,
                            changes,
                            node.max_chars,
                            maybe_filter,
                        );
                    }
                }
                TextInputAction::Copy => {
                    if let Some(text) = editor.copy_selection() {
                        let _ = clipboard.set_text(text);
                    }
                }
                TextInputAction::Paste => {
                    actions_queue.add_front(TextInputAction::PasteDeferred(clipboard.fetch_text()));
                }
                TextInputAction::PasteDeferred(mut clipboard_read) => {
                    if let Some(text) = clipboard_read.poll_result() {
                        if let Ok(text) = text {
                            apply_text_input_edit(
                                TextInputEdit::Paste(text),
                                &mut editor,
                                changes,
                                node.max_chars,
                                maybe_filter,
                            );
                        }
                    } else {
                        // Add the clipboard read back to the queue, process it and the remaining actions next frame.
                        actions_queue.add_front(TextInputAction::PasteDeferred(clipboard_read));
                        break;
                    }
                }
                TextInputAction::Edit(text_input_edit) => {
                    apply_text_input_edit(
                        text_input_edit,
                        &mut editor,
                        changes,
                        node.max_chars,
                        maybe_filter,
                    );
                }
                TextInputAction::ImePreedit { value } => {
                    // Remove previous preedit text if any
                    remove_preedit_text(&mut editor, &mut ime_state);

                    if value.is_empty() {
                        // Composition cancelled
                        ime_state.preedit = None;
                        ime_state.saved_cursor = None;
                        ime_state.preedit_char_count = 0;
                    } else {
                        // Save cursor position before inserting preedit text
                        let char_count = value.chars().count();
                        let saved = editor.cursor();
                        editor.insert_string(&value, None);

                        ime_state.preedit = Some(value);
                        ime_state.saved_cursor = Some(saved);
                        ime_state.preedit_char_count = char_count;
                    }
                    editor.set_redraw(true);
                }
                TextInputAction::ImeCommit { value } => {
                    // Remove preedit text first
                    remove_preedit_text(&mut editor, &mut ime_state);

                    // Insert committed text normally (with undo tracking, filter, max_chars)
                    if !value.is_empty() {
                        apply_text_input_edit(
                            TextInputEdit::Paste(value),
                            &mut editor,
                            changes,
                            node.max_chars,
                            maybe_filter,
                        );
                    }

                    // Clear IME state
                    ime_state.preedit = None;
                    ime_state.saved_cursor = None;
                    ime_state.preedit_char_count = 0;
                }
            }
        }
    }
}

pub fn on_focused_keyboard_input(
    trigger: On<FocusedInput<KeyboardInput>>,
    mut query: Query<(&TextInputNode, &mut TextInputQueue, &TextInputImeState)>,
    mut global_state: ResMut<TextInputGlobalState>,
) {
    if let Ok((input, mut queue, ime_state)) = query.get_mut(trigger.focused_entity) {
        // Suppress keyboard input while IME is composing — the IME handles these keys
        if ime_state.preedit.is_some() {
            let key = &trigger.event().input.logical_key;
            if matches!(
                key,
                Key::Character(_) | Key::Space | Key::Backspace | Key::Delete | Key::Enter
            ) && trigger.event().input.state.is_pressed()
            {
                return;
            }
        }

        let TextInputGlobalState {
            shift,
            overwrite_mode,
            command,
        } = &mut *global_state;
        queue_text_input_action(
            &input.mode,
            shift,
            overwrite_mode,
            command,
            &trigger.event().input,
            |action| {
                queue.add(action);
            },
        );
    }
}

/// Remove previously inserted preedit text from the editor using selection-based deletion.
/// This is more robust than backspace-counting since it doesn't depend on cursor position.
fn remove_preedit_text(
    editor: &mut BorrowedWithFontSystem<Editor>,
    ime_state: &mut TextInputImeState,
) {
    if ime_state.preedit_char_count > 0 {
        if let Some(saved_cursor) = ime_state.saved_cursor {
            // Select from saved cursor (before preedit) to current cursor (after preedit)
            // The current cursor is at the end of the preedit text
            let current_cursor = editor.cursor();
            editor.set_cursor(saved_cursor);
            editor.set_selection(Selection::Normal(current_cursor));
            editor.delete_selection();
        } else {
            // Fallback: use backspace if we don't have a saved cursor
            for _ in 0..ime_state.preedit_char_count {
                editor.action(Action::Backspace);
            }
        }
        ime_state.preedit_char_count = 0;
        ime_state.saved_cursor = None;
    }
}

/// Enables or disables IME on the window based on whether a text input is focused.
/// Sets `ime_position` to just below the text cursor. The position is only updated
/// when no preedit is active, so it stays fixed during composition.
pub fn ime_focus_system(
    input_focus: Res<InputFocus>,
    text_input_query: Query<
        (
            &TextInputBuffer,
            &ComputedNode,
            &UiGlobalTransform,
            &TextInputImeState,
        ),
        With<TextInputNode>,
    >,
    mut window_query: Query<&mut Window>,
) {
    if let Some(focused) = input_focus.0 {
        if let Ok((buffer, node, transform, ime_state)) = text_input_query.get(focused) {
            if let Some(mut window) = window_query.iter_mut().next() {
                window.ime_enabled = true;
                // Only update position when not composing, so the candidate
                // box stays fixed once composition starts.
                if ime_state.preedit.is_none() {
                    if let Some((cx, cy)) = buffer.editor.cursor_position() {
                        let rect = Rect::from_center_size(transform.translation, node.size());
                        let line_height = buffer.editor.with_buffer(|b| b.metrics().line_height);
                        window.ime_position = Vec2::new(
                            rect.min.x + cx as f32,
                            rect.min.y + cy as f32 + line_height * 0.8,
                        );
                    }
                }
            }
            return;
        }
    }
    // No text input focused — disable IME
    for mut window in window_query.iter_mut() {
        if window.ime_enabled {
            window.ime_enabled = false;
        }
    }
}

/// Reads `Ime` messages and routes them to the focused text input's action queue.
pub fn ime_event_system(
    mut ime_reader: MessageReader<Ime>,
    input_focus: Res<InputFocus>,
    mut query: Query<&mut TextInputQueue, With<TextInputNode>>,
) {
    for ime in ime_reader.read() {
        let Some(focused) = input_focus.0 else {
            continue;
        };
        let Ok(mut queue) = query.get_mut(focused) else {
            continue;
        };
        match ime {
            Ime::Preedit { value, .. } => {
                queue.add(TextInputAction::ImePreedit {
                    value: value.clone(),
                });
            }
            Ime::Commit { value, .. } => {
                queue.add(TextInputAction::ImeCommit {
                    value: value.clone(),
                });
            }
            _ => {}
        }
    }
}

/// Cancels preedit text when the text input loses focus.
pub fn ime_cleanup_on_unfocus_system(
    input_focus: Res<InputFocus>,
    mut query: Query<(Entity, &mut TextInputImeState, &mut TextInputBuffer), With<TextInputNode>>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
) {
    for (entity, mut ime_state, mut buffer) in query.iter_mut() {
        if ime_state.preedit.is_some() {
            let is_focused = input_focus.0.is_some_and(|f| f == entity);
            if !is_focused {
                let mut editor = buffer
                    .editor
                    .borrow_with(&mut text_input_pipeline.font_system);
                remove_preedit_text(&mut editor, &mut ime_state);
                ime_state.preedit = None;
                ime_state.saved_cursor = None;
                ime_state.preedit_char_count = 0;
            }
        }
    }
}
