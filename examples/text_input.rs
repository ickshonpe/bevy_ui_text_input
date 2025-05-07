//! text input example

use bevy::{
    color::palettes::css::{LIGHT_GOLDENROD_YELLOW, MAROON, RED},
    prelude::*,
    text::cosmic_text::Align,
};
use bevy_ui_text_input::{
    SubmitTextEvent, TextInputBuffer, TextInputMode, TextInputNode, TextInputPlugin,
    TextInputPrompt, TextInputStyle, TextSubmissionEvent,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TextInputPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (button_system, submit))
        .run();
}

#[derive(Component)]
struct OutputMarker;

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    // UI camera
    commands.spawn(Camera2d);

    let output = commands
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.),
            ..Default::default()
        },))
        .with_children(|commands| {
            commands
                .spawn((
                    Node {
                        border: UiRect::all(Val::Px(2.)),
                        padding: UiRect::all(Val::Px(2.)),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::clip(),
                        ..Default::default()
                    },
                    BorderColor(Color::WHITE),
                    BackgroundColor(Color::BLACK),
                ))
                .with_child((
                    Node {
                        width: Val::Px(500.),
                        height: Val::Px(500.),
                        max_height: Val::Px(500.),
                        min_height: Val::Px(500.),
                        max_width: Val::Px(500.),
                        min_width: Val::Px(500.),
                        ..Default::default()
                    },
                    Text::new("Nothing submitted."),
                    OutputMarker,
                ));
        })
        .id();

    let editor = commands
        .spawn((
            TextInputNode {
                clear_on_submit: true,
                unfocus_on_submit: false,
                ..Default::default()
            },
            TextInputPrompt {
                text: "The TextInputPrompt is shown when the input is empty..".to_string(),
                color: Some(Color::srgb(0.3, 0.3, 0.3)),
                ..Default::default()
            },
            TextInputBuffer::default(),
            TextFont {
                font: assets.load("fonts/FiraSans-Bold.ttf"),
                font_size: 25.,
                ..Default::default()
            },
            TextColor(LIGHT_GOLDENROD_YELLOW.into()),
            Node {
                width: Val::Px(500.),
                height: Val::Px(500.),
                ..default()
            },
            TextInputStyle::default(),
            BackgroundColor(Color::srgb(0., 0., 0.2)),
        ))
        .id();

    let submit_button = commands
        .spawn((
            Node {
                align_self: AlignSelf::Start,
                border: UiRect::all(Val::Px(2.)),
                padding: UiRect::all(Val::Px(2.)),
                ..Default::default()
            },
            BorderColor(Color::WHITE),
            Button,
        ))
        .with_child(Text::new("Submit"))
        .observe(
            move |_: Trigger<Pointer<Click>>, mut submit_writer: EventWriter<SubmitTextEvent>| {
                submit_writer.write(SubmitTextEvent { entity: editor });
            },
        )
        .id();

    let control_panel = commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..Default::default()
        },))
        .add_child(submit_button)
        .with_children(|commands| {
            commands
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Start,
                    align_content: AlignContent::Start,
                    width: Val::Auto,
                    height: Val::Auto,
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    column_gap: Val::Px(4.),
                    ..Default::default()
                })
                .with_children(|commands| {
                    commands
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.),
                            ..Default::default()
                        })
                        .with_children(|commands| {
                            commands
                        .spawn((
                            Node {
                                border: UiRect::all(Val::Px(2.)),
                                padding: UiRect::all(Val::Px(2.)),
                                ..Default::default()
                            },
                            BorderColor(Color::WHITE), Button
                        ))
                        .with_child(Text::new("sans"))
                        .observe(
                            move |_: Trigger<Pointer<Click>>,
                                mut query: Query<&mut TextFont>,
                                assets: Res<AssetServer>| {
                                if let Ok(mut text_font) = query.get_mut(editor) {
                                    text_font.font = assets.load("fonts/FiraSans-Bold.ttf");
                                }
                            },
                        );
                            commands
                        .spawn((
                            Node {
                                border: UiRect::all(Val::Px(2.)),
                                padding: UiRect::all(Val::Px(2.)),
                                ..Default::default()
                            },
                            BorderColor(Color::WHITE), Button
                        ))
                        .observe(
                            move |_: Trigger<Pointer<Click>>,
                                  mut query: Query<&mut TextFont>,
                                  assets: Res<AssetServer>| {
                                if let Ok(mut text_font) = query.get_mut(editor) {
                                    text_font.font = assets.load("fonts/FiraMono-Medium.ttf");
                                }
                            },
                        )
                        .with_child(Text::new("mono"));
                        });
                    commands
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.),
                            ..Default::default()
                        })
                        .with_children(|commands| {
                            commands
                        .spawn((
                            Node {
                                border: UiRect::all(Val::Px(2.)),
                                padding: UiRect::all(Val::Px(2.)),
                                ..Default::default()
                            },
                            BorderColor(Color::WHITE), Button
                        ))
                        .observe(
                            move |_: Trigger<Pointer<Click>>, mut query: Query<&mut TextFont>| {
                                if let Ok(mut text_font) = query.get_mut(editor) {
                                    text_font.font_size = 16.;
                                }
                            },
                        )
                        .with_child(Text::new("16"));

                            commands
                        .spawn((
                            Node {
                                border: UiRect::all(Val::Px(2.)),
                                padding: UiRect::all(Val::Px(2.)),
                                ..Default::default()
                            },
                            BorderColor(Color::WHITE), Button
                        ))
                        .observe(
                            move |_: Trigger<Pointer<Click>>, mut query: Query<&mut TextFont>| {
                                if let Ok(mut text_font) = query.get_mut(editor) {
                                    text_font.font_size = 25.;
                                }
                            },
                        )
                        .with_child(Text::new("25"));
                        });

                    commands
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.),
                            ..Default::default()
                        })
                        .with_children(|commands| {
                            for w in [250, 500] {
                                commands
                                .spawn((
                                    Node {
                                        border: UiRect::all(Val::Px(2.)),
                                        padding: UiRect::all(Val::Px(2.)),
                                        ..Default::default()
                                    },
                                    BorderColor(Color::WHITE), Button
                                ))
                                .observe(
                                    move |_: Trigger<Pointer<Click>>, mut query: Query<&mut Node>| {
                                        if let Ok(mut node) = query.get_mut(editor) {
                                            node.width = Val::Px(w as f32);
                                        }
                                    },
                                )
                                .with_child(Text::new(format!("{w}w")));
                            }
                        });

                    commands
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.),
                            ..Default::default()
                        })
                        .with_children(|commands| {
                            for h in [250, 500] {
                                commands
                            .spawn((
                                Node {
                                    border: UiRect::all(Val::Px(2.)),
                                    padding: UiRect::all(Val::Px(2.)),
                                    ..Default::default()
                                },
                                BorderColor(Color::WHITE), Button
                            ))
                            .observe(
                                move |_: Trigger<Pointer<Click>>, mut query: Query<&mut Node>| {
                                    if let Ok(mut node) = query.get_mut(editor) {
                                        node.height = Val::Px(h as f32);
                                    }
                                },
                            )
                            .with_child(Text::new(format!("{h}h")));
                            }
                        });

                    commands
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.),
                            ..Default::default()
                        })
                        .with_children(|commands| {
                            commands
                            .spawn((
                                Node {
                                    border: UiRect::all(Val::Px(2.)),
                                    padding: UiRect::all(Val::Px(2.)),
                                    ..Default::default()
                                },
                                BorderColor(Color::WHITE), Button
                            ))
                            .observe(
                                move |_: Trigger<Pointer<Click>>, mut query: Query<&mut TextInputNode>| {
                                    if let Ok(mut input) = query.get_mut(editor) {
                                        let wrap = match input.mode.wrap() {
                                            bevy::text::cosmic_text::Wrap::None => bevy::text::cosmic_text::Wrap::Glyph,
                                            bevy::text::cosmic_text::Wrap::Glyph => bevy::text::cosmic_text::Wrap::Word,
                                            bevy::text::cosmic_text::Wrap::Word => bevy::text::cosmic_text::Wrap::WordOrGlyph,
                                            bevy::text::cosmic_text::Wrap::WordOrGlyph => bevy::text::cosmic_text::Wrap::None,
                                        };
                                        input.mode = TextInputMode::Text { wrap };
                                    }
                                },
                            )
                            .with_child(Text::new(format!("wrap")));
                        });

                        commands
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.),
                            ..Default::default()
                        })
                        .with_children(|commands| {
                            commands
                            .spawn((
                                Node {
                                    border: UiRect::all(Val::Px(2.)),
                                    padding: UiRect::all(Val::Px(2.)),
                                    ..Default::default()
                                },
                                BorderColor(Color::WHITE), Button
                            ))
                            .observe(
                                move |_: Trigger<Pointer<Click>>, mut query: Query<&mut TextInputNode>| {
                                    if let Ok(mut input) = query.get_mut(editor) {
                                        input.alignment = match input.alignment {
                                            Some(align) => match align {
                                                Align::Left => Some(Align::Right),
                                                Align::Right => Some(Align::Center),
                                                Align::Center => Some(Align::Justified),
                                                Align::Justified => Some(Align::End),
                                                Align::End => None,
                                            },
                                            None => Some(Align::Left),
                                        }
                                    }
                                },
                            )
                            .with_child(Text::new(format!("align")));
                        });
                });
        })
        .id();

    let editor_panel = commands
        .spawn(Node {
            width: Val::Px(504.),
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Start,
            ..Default::default()
        })
        .with_children(|commands| {
            commands
                .spawn((
                    Node {
                        border: UiRect::all(Val::Px(2.)),
                        padding: UiRect::all(Val::Px(2.)),
                        ..Default::default()
                    },
                    BorderColor(Color::WHITE),
                    BackgroundColor(Color::BLACK),
                ))
                .add_child(editor);
        })
        .id();

    commands
        .spawn(Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            margin: UiRect::top(Val::Px(10.)),
            row_gap: Val::Px(10.),
            column_gap: Val::Px(10.),
            ..Default::default()
        })
        .with_child(Text::new("Text Input Example".to_string()))
        .with_children(|commands| {
            commands
                .spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: vec![GridTrack::auto(), GridTrack::auto()],
                    column_gap: Val::Px(10.),
                    row_gap: Val::Px(10.),
                    ..Default::default()
                })
                .add_child(editor_panel)
                .add_child(output)
                .add_child(control_panel)
                .with_child(Text::new(
                    "Press Shift + Enter or click the button to submit",
                ));
        });
}

fn button_system(
    mut interaction_query: Query<
        (&Interaction, &mut BorderColor, &Children),
        (Changed<Interaction>, With<Button>),
    >,
    mut text_query: Query<&mut TextColor>,
) {
    for (interaction, mut border_color, children) in &mut interaction_query {
        let mut color = text_query.get_mut(children[0]).unwrap();
        match *interaction {
            Interaction::Pressed => {
                color.0 = MAROON.into();
                border_color.0 = MAROON.into();
            }
            Interaction::Hovered => {
                color.0 = RED.into();
                border_color.0 = RED.into();
            }
            Interaction::None => {
                color.0 = Color::WHITE;
                border_color.0 = Color::WHITE;
            }
        }
    }
}

fn submit(
    mut events: EventReader<TextSubmissionEvent>,
    mut query: Query<&mut Text, With<OutputMarker>>,
) {
    for event in events.read() {
        for mut text in query.iter_mut() {
            text.0 = event.text.clone();
        }
    }
}
