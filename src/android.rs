use bevy::prelude::*;
use bevy::input_focus::InputFocus;
use bevy_android::ANDROID_APP;
use android_activity::input::{TextInputState, TextSpan};
use crate::{TextInputBuffer, TextInputGlobalState, TextInputNode, TextInputQueue};
use crate::actions::{TextInputAction, TextInputEdit};

#[cfg(target_os = "android")]
pub fn android_text_input_poll_system(
    focus: Res<InputFocus>,
    mut global_state: ResMut<TextInputGlobalState>,
    mut query: Query<&mut TextInputQueue, With<TextInputNode>>,
) {
    let Some(app) = ANDROID_APP.get() else { return; };
    let Some(focused_entity) = focus.0 else { return; };

    if let Ok(mut queue) = query.get_mut(focused_entity) {
        let current_state = app.text_input_state();
        
        if current_state.text != global_state.last_android_text {
            enqueue_text_diff(&global_state.last_android_text, &current_state.text, &mut queue);
            global_state.last_android_text = current_state.text.clone();
        }
    }
}

#[cfg(target_os = "android")]
pub fn android_text_input_sync_system(
    focus: Res<InputFocus>,
    mut global_state: ResMut<TextInputGlobalState>,
    query: Query<(&TextInputBuffer, &TextInputQueue), (With<TextInputNode>, Changed<TextInputBuffer>)>,
) {
    let Some(app) = ANDROID_APP.get() else { return; };
    let Some(focused_entity) = focus.0 else { return; };

    if let Ok((buffer, queue)) = query.get(focused_entity) {
        if !queue.is_empty() {
            return;
        }

        let bevy_text = buffer.get_text();

        if bevy_text != global_state.last_android_text {
            let state = TextInputState {
                text: bevy_text.clone(),
                selection: TextSpan { start: bevy_text.len(), end: bevy_text.len() },
                compose_region: None,
            };
            app.set_text_input_state(state);
            global_state.last_android_text = bevy_text;
        }
    }
}

fn enqueue_text_diff(prev: &str, new: &str, queue: &mut TextInputQueue) {
    let prev_chars: Vec<char> = prev.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();

    let common_prefix = prev_chars
        .iter()
        .zip(new_chars.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let prev_tail = &prev_chars[common_prefix..];
    let new_tail = &new_chars[common_prefix..];

    let common_suffix = prev_tail
        .iter()
        .rev()
        .zip(new_tail.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    let deletions = prev_tail.len() - common_suffix;
    let insertions = &new_tail[..new_tail.len() - common_suffix];

    for _ in 0..deletions {
        if !queue.actions.iter().any(|a| matches!(a, TextInputAction::Edit(TextInputEdit::Backspace))) {
             queue.add(TextInputAction::Edit(TextInputEdit::Backspace));
        }
    }

    for &ch in insertions {
        let is_problematic = ch.is_ascii_digit() || ch == ' ';
        
        if is_problematic {
            let already_in_queue = queue.actions.iter().any(|a| {
                if let TextInputAction::Edit(TextInputEdit::Insert(existing_ch, _)) = a {
                    *existing_ch == ch
                } else {
                    false
                }
            });

            if already_in_queue {
                continue; 
            }
        }

        queue.add(TextInputAction::Edit(TextInputEdit::Insert(ch, false)));
    }
}

#[cfg(target_os = "android")]
pub fn on_text_input_pressed_android(
    trigger: On<Pointer<Press>>,
    node_query: Query<&TextInputNode>,
) {
    if trigger.button != PointerButton::Primary {
        return;
    }

    let Ok(input) = node_query.get(trigger.entity) else {
        return;
    };

    if !input.is_enabled || !input.focus_on_pointer_down {
        return;
    }

    let Some(app) = ANDROID_APP.get() else { return; };
    app.show_soft_input(true);
}