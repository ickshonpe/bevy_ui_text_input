//! minimal text input example

use bevy::{
    color::palettes::css::NAVY,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    window::{PresentMode, WindowResolution},
};
use bevy_ui_text_input::{TextInputNode, TextInputPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    present_mode: PresentMode::AutoNoVsync,
                    resolution: WindowResolution::new(1920.0, 1080.0)
                        .with_scale_factor_override(1.0),
                    ..default()
                }),
                ..default()
            }),
            TextInputPlugin,
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, fps_system)
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
            TextInputNode::default(),
            Node {
                width: Val::Px(500.),
                height: Val::Px(250.),
                ..default()
            },
            BackgroundColor(NAVY.into()),
        ));
}

fn fps_system(diagnostics: Res<DiagnosticsStore>) {
    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .map(|fps| fps.smoothed())
    {
        info!("fps: {fps:?}");
    }
}
