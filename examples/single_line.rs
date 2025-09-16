//! minimal text input example

use bevy::{color::palettes::css::NAVY, prelude::*};
use bevy_ui_text_input::{
    SubmitText, TextInputMode, TextInputNode, TextInputPlugin, TextInputPrompt,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TextInputPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
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
            column_gap: Val::Px(20.),
            ..Default::default()
        })
        .with_child((
            TextInputNode {
                mode: TextInputMode::SingleLine,
                max_chars: Some(20),
                clear_on_submit: true,
                ..Default::default()
            },
            TextFont {
                font: assets.load("fonts/FiraMono-Medium.ttf"),
                font_size: 25.,
                ..Default::default()
            },
            TextInputPrompt::default(),
            Node {
                width: Val::Px(250.),
                height: Val::Px(25.),
                ..default()
            },
            BackgroundColor(NAVY.into()),
        ))
        .with_child(Text::new("submit something.."));
}

fn update(mut events: MessageReader<SubmitText>, mut query: Query<&mut Text>) {
    for event in events.read() {
        for mut text in query.iter_mut() {
            text.0 = event.text.clone();
        }
    }
}
