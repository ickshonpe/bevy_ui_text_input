use crate::TextInputBuffer;
use crate::TextInputGlobalState;
use crate::TextInputMode;
use crate::TextInputNode;
use crate::TextInputQueue;
use crate::TextInputStyle;
use crate::TextSubmitEvent;
use crate::actions::TextInputAction;
use crate::actions::TextInputEdit;
use crate::actions::apply_text_input_edit;
use crate::clipboard::Clipboard;
use crate::text_input_pipeline::TextInputPipeline;
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::event::EventWriter;
use bevy::ecs::observer::Trigger;
use bevy::ecs::query::With;
use bevy::ecs::system::Commands;
use bevy::ecs::system::Local;
use bevy::ecs::system::NonSend;
use bevy::ecs::system::Query;
use bevy::ecs::system::Res;
use bevy::ecs::system::ResMut;
use bevy::ecs::system::Single;
use bevy::input::ButtonState;
use bevy::input::keyboard::Key;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::mouse::MouseWheel;
use bevy::input_focus::FocusedInput;
use bevy::input_focus::InputFocus;
use bevy::log::warn;
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
use bevy::window::Ime;
use bevy::window::Window;
use bevy::winit::WinitWindows;

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

pub fn buffer_len(buffer: &bevy::text::cosmic_text::Buffer) -> usize {
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

pub(crate) fn is_buffer_empty(buffer: &bevy::text::cosmic_text::Buffer) -> bool {
    buffer.lines.len() == 0 || (buffer.lines.len() == 1 && buffer.lines[0].text().is_empty())
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
    click: Trigger<Pointer<Click>>,
    time: Res<Time>,
    mut text_input_nodes: Query<(
        &TextInputNode,
        &mut TextInputQueue,
        &mut TextInputBuffer,
        &GlobalTransform,
        &ComputedNode,
    )>,
    mut multi_click_datas: Query<&mut MultiClickData>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    mut commands: Commands,
) {
    if click.button != PointerButton::Primary {
        return;
    }

    let entity = click.target();

    let Ok((input, mut queue, mut buffer, transform, node)) = text_input_nodes.get_mut(entity)
    else {
        return;
    };

    if !input.is_enabled || !input.focus_on_pointer_down {
        return;
    }

    let now = time.elapsed_secs();
    if let Ok(mut multi_click_data) = multi_click_datas.get_mut(entity) {
        if now - multi_click_data.last_click_time
            <= MULTI_CLICK_PERIOD * multi_click_data.click_count as f32
        {
            let rect = Rect::from_center_size(transform.translation().truncate(), node.size());

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
    }
    if let Ok(mut entity) = commands.get_entity(entity) {
        entity.try_insert(MultiClickData {
            last_click_time: now,
            click_count: 1,
        });
    }
}

pub fn on_move_clear_multi_click(move_: Trigger<Pointer<Move>>, mut commands: Commands) {
    if let Ok(mut entity) = commands.get_entity(move_.target()) {
        entity.try_remove::<MultiClickData>();
    }
}

pub fn queue_text_input_action(
    input_mode: &TextInputMode,
    shift_pressed: &mut bool,
    overwrite_mode: &mut bool,
    command_pressed: &mut bool,
    keyboard_input: &KeyboardInput,
    mut queue: impl FnMut(TextInputAction) -> (),
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
    )>,
    mut text_input_pipeline: ResMut<TextInputPipeline>,
    mut submit_writer: EventWriter<TextSubmitEvent>,
    mut clipboard: ResMut<Clipboard>,
) {
    let mut font_system = &mut text_input_pipeline.font_system;

    for (entity, node, mut buffer, mut actions_queue) in query.iter_mut() {
        let TextInputBuffer {
            editor, changes, ..
        } = &mut *buffer;
        let mut editor = editor.borrow_with(&mut font_system);
        while let Some(action) = actions_queue.next() {
            match action {
                TextInputAction::Submit => {
                    let text = editor.with_buffer(crate::get_text);
                    submit_writer.write(TextSubmitEvent { entity, text });
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
                            &node.filter,
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
                                &node.filter,
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
                        &node.filter,
                    );
                }
            }
        }
    }
}

pub fn on_focused_keyboard_input(
    trigger: Trigger<FocusedInput<KeyboardInput>>,
    mut query: Query<(&TextInputNode, &mut TextInputQueue)>,
    mut global_state: ResMut<TextInputGlobalState>,
) {
    if let Ok((input, mut queue)) = query.get_mut(trigger.target()) {
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

pub fn listen_ime_events(
    trigger: Trigger<Ime>,
    mut text_inputs: Query<&mut TextInputQueue, With<TextInputNode>>,
    mut global_state: ResMut<TextInputGlobalState>,
) {
    let Ok(mut queue) = text_inputs.get_mut(trigger.target()) else {
        return;
    };

    let TextInputGlobalState { overwrite_mode, .. } = &mut *global_state;

    if let Ime::Commit { value, .. } = trigger.event() {
        for character in value.chars() {
            queue.add(TextInputAction::Edit(TextInputEdit::Insert(
                character,
                *overwrite_mode,
            )));
        }
    }
}

pub fn toggle_ime_on_focus(
    input_focus: Res<InputFocus>,
    text_inputs: Query<&TextInputNode>,
    // supporting multiple windows requires detecting
    // on which window the text input is currently on.
    // because IME is mostly useful on mobile,
    // it's ok to only support single window for now.
    window: Single<Entity, With<Window>>,
    winit_windows: NonSend<WinitWindows>,
    mut ime_allowed: Local<bool>,
) {
    if input_focus.is_changed() {
        let is_text_input_focused = input_focus
            .get()
            .is_some_and(|focused_entity| text_inputs.get(focused_entity).is_ok());

        if is_text_input_focused != *ime_allowed {
            let Some(window) = winit_windows.get_window(*window) else {
                warn!("no window found to toggle IME on");
                return;
            };

            *ime_allowed = is_text_input_focused;
            window.set_ime_allowed(*ime_allowed);
        }
    }
}
