//! minimal text input example

use bevy::{color::palettes::css::NAVY, input_focus::InputFocus, prelude::*};
use bevy_ui_text_input::{
    TextInputMode, TextInputNode, TextInputPlugin, TextSubmitEvent,
};
use once_cell::sync::Lazy;
use regex::Regex;

static FILTER_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^-?$|^-?\d*\.?\d*$").unwrap());

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TextInputPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, reciever)
        .run();
}

fn setup(mut commands: Commands, mut active_input: ResMut<InputFocus>) {
    // UI camera
    commands.spawn(Camera2d);

    let input_entity = commands
        .spawn((
            TextInputNode {
                mode: TextInputMode::SingleLine,
                filter: Some(Box::new(|text| FILTER_REGEX.is_match(text))),
                max_chars: Some(10),
                ..Default::default()
            },
            TextFont {
                font_size: 20.,
                ..Default::default()
            },
            Node {
                width: Val::Px(100.),
                height: Val::Px(20.),
                ..default()
            },
            BackgroundColor(NAVY.into()),
        ))
        .id();

    active_input.set(input_entity);

    commands
        .spawn(Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.),
            ..Default::default()
        })
        .add_child(input_entity);
}

fn reciever(mut events: EventReader<TextSubmitEvent>) {
    for event in events.read() {
        let d: f64 = event.text.parse().unwrap();
        println!("decimal: {}", d);
    }
}
