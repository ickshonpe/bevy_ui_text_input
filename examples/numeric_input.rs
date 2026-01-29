//! minimal text input example

use bevy::{color::palettes::css::NAVY, prelude::*};
use bevy_ui_text_input::{TextInputFilter, TextInputMode, TextInputNode, TextInputPlugin};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TextInputPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // UI camera
    commands.spawn(Camera2d);

    let input_node = commands
        .spawn((
            TextInputNode {
                mode: TextInputMode::SingleLine,
                max_chars: Some(5),
                ..Default::default()
            },
            TextInputFilter::Integer,
            Node {
                width: Val::Px(500.),
                height: Val::Px(250.),
                ..default()
            },
            BackgroundColor(NAVY.into()),
        ))
        .id();

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
        .add_child(input_node);
}
