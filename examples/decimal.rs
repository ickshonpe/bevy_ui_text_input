//! minimal text input example

use bevy::{color::palettes::css::NAVY, prelude::*};
use bevy_ui_text_input::{TextInputMode, TextInputNode, TextInputPlugin, TextSubmissionEvent};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TextInputPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, reciever)
        .run();
}

fn setup(mut commands: Commands) {
    // UI camera
    commands.spawn(Camera2d);
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
        .with_child((
            TextInputNode {
                is_active: true,
                mode: TextInputMode::Decimal,
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
        ));
}

fn reciever(mut events: EventReader<TextSubmissionEvent>) {
    for event in events.read() {
        let d: f64 = event.text.parse().unwrap();
        println!("decimal: {}", d);
    }
}
