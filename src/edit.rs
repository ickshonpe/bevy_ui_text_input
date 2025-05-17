use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::event::EventWriter;
use bevy::ecs::observer::Trigger;
use bevy::ecs::system::Commands;
use bevy::ecs::system::Local;
use bevy::ecs::system::Query;
use bevy::ecs::system::Res;
use bevy::ecs::system::ResMut;
use bevy::input::ButtonState;
use bevy::input::keyboard::Key;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::mouse::MouseWheel;
use bevy::input_focus::InputFocus;
use bevy::math::Rect;
use bevy::picking::events::Click;
use bevy::picking::events::Drag;
use bevy::picking::events::Move;
use bevy::picking::events::Pointer;
use bevy::picking::events::Pressed;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerButton;
use bevy::text::cosmic_text::Action;
use bevy::text::cosmic_text::BorrowedWithFontSystem;
use bevy::text::cosmic_text::Change;
use bevy::text::cosmic_text::Edit;
use bevy::text::cosmic_text::Editor;
use bevy::text::cosmic_text::Motion;
use bevy::text::cosmic_text::Selection;
use bevy::time::Time;
use bevy::transform::components::GlobalTransform;
use bevy::ui::ComputedNode;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::SubmitTextEvent;
use crate::TextInputBuffer;
use crate::TextInputMode;
use crate::TextInputNode;
use crate::TextInputStyle;
use crate::TextSubmissionEvent;
use crate::clipboard::Clipboard;
use crate::clipboard::ClipboardContents;
use crate::text_input_pipeline::TextInputPipeline;

static INTEGER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^-?$|^-?\d+$").unwrap());
static DECIMAL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^-?$|^-?\d*\.?\d*$").unwrap());

fn apply_action<'a>(
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

fn apply_motion<'a>(
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

fn filter_char_input(mode: TextInputMode, ch: char) -> bool {
    match mode {
        TextInputMode::TextSingleLine => ch != '\n',
        TextInputMode::Text { .. } => {
            // Allow all characters for text mode
            true
        }
        TextInputMode::Integer => {
            // Allow only numeric characters
            ch.is_ascii_digit() || ch == '-'
        }
        TextInputMode::Hex => {
            // Allow hexadecimal characters (0-9, a-f, A-F)
            ch.is_ascii_hexdigit()
        }
        TextInputMode::Decimal => {
            // Allow numeric characters and a single decimal point
            ch.is_ascii_digit() || ch == '.' || ch == '-'
        }
    }
}

fn filter_text(mode: TextInputMode, text: &str) -> bool {
    matches!(mode, TextInputMode::Text { .. }) || text.chars().all(|ch| filter_char_input(mode, ch))
}

fn buffer_len(buffer: &bevy::text::cosmic_text::Buffer) -> usize {
    buffer
        .lines
        .iter()
        .map(|line| line.text().chars().count())
        .sum()
}

fn cursor_at_line_end(editor: &mut BorrowedWithFontSystem<Editor<'_>>) -> bool {
    let cursor = editor.cursor();
    editor.with_buffer(|buffer| {
        buffer
            .lines
            .get(cursor.line)
            .map(|line| cursor.index == line.text().len())
            .unwrap_or(false)
    })
}

pub(crate) fn is_buffer_empty(buffer: &bevy::text::cosmic_text::Buffer) -> bool {
    buffer.lines.len() == 0 || (buffer.lines.len() == 1 && buffer.lines[0].text().is_empty())
}

pub fn text_input_edit_system(
    mut clipboard_queue: Local<Vec<ClipboardContents>>,
    mut shift_pressed: Local<bool>,
    mut command_pressed: Local<bool>,
    mut keyboard_events_reader: EventReader<KeyboardInput>,
    mut query: Query<(
        Entity,
        &TextInputNode,
        &mut TextInputBuffer,
        &TextInputStyle,
    )>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    mut submit_reader: EventReader<SubmitTextEvent>,
    mut submit_writer: EventWriter<TextSubmissionEvent>,
    mut input_focus: ResMut<InputFocus>,
    mut clipboard: ResMut<Clipboard>,
    time: Res<Time>,
) {
    let Some(entity) = input_focus.0 else {
        return;
    };

    let Ok((entity, input, mut buffer, style)) = query.get_mut(entity) else {
        return;
    };

    let keyboard_events: Vec<_> = keyboard_events_reader.read().collect();

    let mut font_system = &mut text_input_pipeline.font_system;

    buffer.cursor_blink_time = if keyboard_events.is_empty() {
        (buffer.cursor_blink_time + time.delta_secs()).rem_euclid(style.blink_interval * 2.)
    } else {
        0.
    };

    let TextInputBuffer {
        editor,
        overwrite_mode,
        changes,
        ..
    } = &mut *buffer;

    let mut editor = editor.borrow_with(&mut font_system);

    let mut remaining = vec![];
    for item in (*clipboard_queue).drain(..) {
        if let Some(Ok(text)) = item.get_or_poll() {
            if input
                .max_chars
                .is_none_or(|max| editor.with_buffer(buffer_len) + text.len() <= max)
            {
                if filter_text(input.mode, &text) {
                    editor.insert_string(&text, None);
                }
            }
        } else {
            remaining.push(item);
        }
    }
    *clipboard_queue = remaining;

    if editor.with_buffer(|buffer| buffer.wrap() != input.mode.wrap()) {
        apply_motion(&mut editor, *shift_pressed, Motion::BufferStart);
        editor.action(Action::Escape);

        editor.with_buffer_mut(|buffer| {
            buffer.set_wrap(input.mode.wrap());
        });
    }

    for event in &keyboard_events {
        if input_focus.0.is_none() {
            break;
        }

        editor.start_change();

        match event.logical_key {
            Key::Shift => {
                *shift_pressed = event.state == ButtonState::Pressed;
                continue;
            }
            Key::Control => {
                *command_pressed = event.state == ButtonState::Pressed;
                continue;
            }
            #[cfg(target_os = "macos")]
            Key::Super => {
                *command_pressed = event.state == ButtonState::Pressed;
                continue;
            }
            _ => {}
        };
        if event.state.is_pressed() {
            if *command_pressed {
                match &event.logical_key {
                    Key::Character(str) => {
                        if let Some(char) = str.chars().next() {
                            // onvert to lowercase so that the commands work when capslock is on
                            match (char.to_ascii_lowercase(), *shift_pressed) {
                                ('c', false) => {
                                    // copy
                                    if let Some(text) = editor.copy_selection() {
                                        let _ = clipboard.set_text(text);
                                    }
                                }
                                ('x', false) => {
                                    // cut
                                    if let Some(text) = editor.copy_selection() {
                                        let _ = clipboard.set_text(text);
                                    }

                                    if editor.delete_selection() {
                                        editor.set_redraw(true);
                                    }
                                }
                                ('v', false) => {
                                    // paste
                                    let contents = clipboard.fetch_text();
                                    if let Some(Ok(text)) = contents.get_or_poll() {
                                        if input.max_chars.is_none_or(|max| {
                                            editor.with_buffer(buffer_len) + text.len() <= max
                                        }) {
                                            if filter_text(input.mode, &text) {
                                                editor.insert_string(&text, None);
                                            }
                                        }
                                    } else {
                                        clipboard_queue.push(contents);
                                    }
                                }
                                ('z', false) => {
                                    for action in changes.undo() {
                                        apply_action(&mut editor, action);
                                    }
                                }
                                #[cfg(target_os = "macos")]
                                ('z', true) => {
                                    for action in changes.redo() {
                                        apply_action(&mut editor, action);
                                    }
                                }
                                ('y', false) => {
                                    for action in changes.redo() {
                                        apply_action(&mut editor, action);
                                    }
                                }
                                ('a', false) => {
                                    // select all
                                    editor.action(Action::Motion(Motion::BufferStart));
                                    let cursor = editor.cursor();
                                    editor.set_selection(Selection::Normal(cursor));
                                    editor.action(Action::Motion(Motion::BufferEnd));
                                }
                                _ => {
                                    // not recognised, ignore
                                }
                            }
                        }
                    }
                    Key::ArrowLeft => {
                        apply_motion(&mut editor, *shift_pressed, Motion::PreviousWord);
                    }
                    Key::ArrowRight => {
                        apply_motion(&mut editor, *shift_pressed, Motion::NextWord);
                    }
                    Key::ArrowUp => {
                        if matches!(input.mode, TextInputMode::Text { .. }) {
                            editor.action(Action::Scroll { lines: -1 });
                        }
                    }
                    Key::ArrowDown => {
                        if matches!(input.mode, TextInputMode::Text { .. }) {
                            editor.action(Action::Scroll { lines: 1 });
                        }
                    }
                    Key::Home => {
                        apply_motion(&mut editor, *shift_pressed, Motion::BufferStart);
                    }
                    Key::End => {
                        apply_motion(&mut editor, *shift_pressed, Motion::BufferEnd);
                    }
                    _ => {
                        // not recognised, ignore
                    }
                }
            } else {
                let mut key = event.logical_key.clone();
                if event.logical_key == Key::Space {
                    key = Key::Character(" ".into());
                }
                match key {
                    Key::Character(str) => {
                        if let Some(char) = str
                            .chars()
                            .next()
                            .filter(|ch| filter_char_input(input.mode, *ch))
                        {
                            if editor.selection() != Selection::None {
                                editor.action(Action::Insert(char));
                            } else if *overwrite_mode && !cursor_at_line_end(&mut editor) {
                                editor.action(Action::Delete);
                                editor.action(Action::Insert(char));
                            } else if input
                                .max_chars
                                .is_none_or(|max_chars| editor.with_buffer(buffer_len) < max_chars)
                            {
                                editor.action(Action::Insert(char));
                                let re = match input.mode {
                                    TextInputMode::Integer => Some(&INTEGER_RE),
                                    TextInputMode::Decimal => Some(&DECIMAL_RE),
                                    _ => None,
                                };
                                if let Some(re) = re {
                                    let text = editor.with_buffer(crate::get_text);
                                    if !re.is_match(&text) {
                                        editor.action(Action::Backspace);
                                    }
                                }
                            }
                        }
                    }
                    Key::Enter => match (*shift_pressed, input.mode) {
                        (false, TextInputMode::Text { .. }) => {
                            editor.action(Action::Enter);
                        }
                        _ => {
                            let text = editor.with_buffer(crate::get_text);
                            submit_writer.write(TextSubmissionEvent { entity, text });

                            if input.clear_on_submit {
                                editor.action(Action::Motion(Motion::BufferStart));
                                let cursor = editor.cursor();
                                editor.set_selection(Selection::Normal(cursor));
                                editor.action(Action::Motion(Motion::BufferEnd));
                                editor.action(Action::Delete);
                            }

                            if input.unfocus_on_submit {
                                input_focus.clear();
                            }
                        }
                    },
                    Key::Backspace => {
                        if editor.delete_selection() {
                            editor.set_redraw(true);
                        } else {
                            editor.action(Action::Backspace);
                        }
                    }
                    Key::Delete => {
                        if *shift_pressed {
                            // cut
                            if let Some(text) = editor.copy_selection() {
                                let _ = clipboard.set_text(text);
                            }

                            if editor.delete_selection() {
                                editor.set_redraw(true);
                            }
                        } else if editor.delete_selection() {
                            editor.set_redraw(true);
                        } else {
                            editor.action(Action::Delete);
                        }
                    }
                    Key::PageUp => {
                        apply_motion(&mut editor, *shift_pressed, Motion::PageUp);
                    }
                    Key::PageDown => {
                        apply_motion(&mut editor, *shift_pressed, Motion::PageDown);
                    }
                    Key::ArrowLeft => {
                        apply_motion(&mut editor, *shift_pressed, Motion::Left);
                    }
                    Key::ArrowRight => {
                        apply_motion(&mut editor, *shift_pressed, Motion::Right);
                    }
                    Key::ArrowUp => {
                        apply_motion(&mut editor, *shift_pressed, Motion::Up);
                    }
                    Key::ArrowDown => {
                        apply_motion(&mut editor, *shift_pressed, Motion::Down);
                    }
                    Key::Home => {
                        apply_motion(&mut editor, *shift_pressed, Motion::Home);
                    }
                    Key::End => {
                        apply_motion(&mut editor, *shift_pressed, Motion::End);
                    }
                    Key::Escape => {
                        editor.action(Action::Escape);
                    }
                    Key::Tab => {
                        if matches!(input.mode, TextInputMode::Text { .. }) {
                            if *shift_pressed {
                                editor.action(Action::Unindent);
                            } else {
                                editor.action(Action::Indent);
                            }
                        }
                    }
                    Key::Insert => {
                        *overwrite_mode = !*overwrite_mode;
                    }
                    _ => {}
                }
            }
        }

        if let Some(change) = editor.finish_change() {
            if !change.items.is_empty() {
                changes.push(change);
                editor.set_redraw(true);
            }
        }
    }

    for SubmitTextEvent { entity } in submit_reader.read() {
        let Ok((_, input, mut editor, _)) = query.get_mut(*entity) else {
            continue;
        };
        let text = editor.editor.with_buffer(crate::get_text);
        submit_writer.write(TextSubmissionEvent {
            entity: *entity,
            text,
        });

        if input.clear_on_submit {
            let mut editor = editor.editor.borrow_with(&mut font_system);
            editor.action(Action::Motion(Motion::BufferStart));
            let cursor = editor.cursor();
            editor.set_selection(Selection::Normal(cursor));
            editor.action(Action::Motion(Motion::BufferEnd));
            editor.action(Action::Delete);
        }
    }
}

pub(crate) fn on_drag_text_input(
    trigger: Trigger<Pointer<Drag>>,
    mut node_query: Query<(
        &ComputedNode,
        &GlobalTransform,
        &mut TextInputBuffer,
        &TextInputNode,
    )>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    input_focus: Res<InputFocus>,
) {
    if trigger.button != PointerButton::Primary {
        return;
    }

    if !input_focus
        .0
        .is_some_and(|input_focus_entity| input_focus_entity == trigger.target)
    {
        return;
    }

    let Ok((node, transform, mut buffer, input)) = node_query.get_mut(trigger.target) else {
        return;
    };

    if !input.is_enabled || !input.focus_on_pointer_down {
        return;
    }

    let rect = Rect::from_center_size(transform.translation().truncate(), node.size());

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
    trigger: Trigger<Pointer<Pressed>>,
    mut node_query: Query<(
        &ComputedNode,
        &GlobalTransform,
        &mut TextInputBuffer,
        &TextInputNode,
    )>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    mut input_focus: ResMut<InputFocus>,
) {
    if trigger.button != PointerButton::Primary {
        return;
    }

    let Ok((node, transform, mut buffer, input)) = node_query.get_mut(trigger.target) else {
        return;
    };

    if !input.is_enabled || !input.focus_on_pointer_down {
        return;
    }

    if !input_focus
        .get()
        .is_some_and(|active_input| active_input == trigger.target)
    {
        input_focus.set(trigger.target);
    }

    let rect = Rect::from_center_size(transform.translation().truncate(), node.size());

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
    mut mouse_wheel_events: EventReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    mut node_query: Query<(&mut TextInputBuffer, &TextInputNode)>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
) {
    for mouse_wheel_event in mouse_wheel_events.read() {
        for (_, pointer_map) in hover_map.iter() {
            for (entity, _) in pointer_map.iter() {
                let Ok((mut buffer, input)) = node_query.get_mut(*entity) else {
                    continue;
                };

                if !matches!(input.mode, TextInputMode::Text { .. }) {
                    continue;
                }

                match mouse_wheel_event.unit {
                    MouseScrollUnit::Line => {
                        let mut editor = buffer
                            .editor
                            .borrow_with(&mut text_input_pipeline.font_system);

                        editor.action(Action::Scroll {
                            lines: -mouse_wheel_event.y as i32,
                        });
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

pub fn clear_selection_on_focus_change(
    input_focus: Res<InputFocus>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    mut buffers: Query<&mut TextInputBuffer>,
    mut previous_input_focus: Local<Option<Entity>>,
) {
    if *previous_input_focus != input_focus.0 {
        if let Some(entity) = *previous_input_focus {
            if let Ok(mut buffer) = buffers.get_mut(entity) {
                buffer
                    .editor
                    .borrow_with(&mut text_input_pipeline.font_system)
                    .set_selection(Selection::None);
            }
        }
        *previous_input_focus = input_focus.0;
    }
}

const MULTI_CLICK_TIMER: f32 = 0.5; // seconds

#[derive(Component)]
pub struct MultiClickData {
    last_click_time: f32,
    times: usize,
}

pub fn on_multi_click_set_selection(
    click: Trigger<Pointer<Click>>,
    time: Res<Time>,
    text_input_nodes: Query<&TextInputNode>,
    mut multi_click_datas: Query<&mut MultiClickData>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    mut buffers: Query<&mut TextInputBuffer>,
    mut commands: Commands,
) {
    if click.button != PointerButton::Primary {
        return;
    }

    let entity = click.target();

    let Ok(input) = text_input_nodes.get(entity) else {
        return;
    };

    if !input.is_enabled || !input.focus_on_pointer_down {
        return;
    }

    let now = time.elapsed_secs();
    if let Ok(mut multi_click_data) = multi_click_datas.get_mut(entity) {
        if now - multi_click_data.last_click_time <= MULTI_CLICK_TIMER {
            if let Ok(mut buffer) = buffers.get_mut(entity) {
                let mut editor = buffer
                    .editor
                    .borrow_with(&mut text_input_pipeline.font_system);
                match multi_click_data.times {
                    1 => {
                        multi_click_data.times += 1;
                        multi_click_data.last_click_time = now;
                        editor.action(Action::Motion(Motion::LeftWord));
                        let cursor = editor.cursor();
                        editor.set_selection(Selection::Normal(cursor));
                        editor.action(Action::Motion(Motion::RightWord));
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
        }
    }
    if let Ok(mut entity) = commands.get_entity(entity) {
        entity.try_insert(MultiClickData {
            last_click_time: now,
            times: 1,
        });
    }
}

pub fn on_move_clear_multi_click(move_: Trigger<Pointer<Move>>, mut commands: Commands) {
    if let Ok(mut entity) = commands.get_entity(move_.target()) {
        entity.try_remove::<MultiClickData>();
    }
}
