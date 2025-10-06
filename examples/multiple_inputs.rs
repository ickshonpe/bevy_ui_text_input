//! minimal text input example

use bevy::{
    color::palettes::css::{GREY, LIGHT_GOLDENROD_YELLOW},
    input_focus::InputFocus,
    platform::collections::HashMap,
    prelude::*,
};
use bevy_ui_text_input::{
    SubmitText, TextInputFilter, TextInputMode, TextInputNode, TextInputPlugin, TextInputPrompt,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TextInputPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        .run();
}

#[derive(Resource, Deref, DerefMut, Default)]
struct InputMap(HashMap<Entity, Entity>);

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    // UI camera
    commands.spawn(Camera2d);

    let mut map = InputMap::default();

    let filters = [
        (None, "text"),
        (Some(TextInputFilter::Integer), "integer"),
        (Some(TextInputFilter::Decimal), "decimal"),
        (Some(TextInputFilter::Hex), "hex"),
    ];

    commands
        .spawn(Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            ..Default::default()
        })
        .with_children(|commands| {
            commands
                .spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: vec![GridTrack::auto(), GridTrack::px(300.)],
                    column_gap: Val::Px(20.),
                    row_gap: Val::Px(20.),
                    ..Default::default()
                })
                .with_children(|commands| {
                    for (filter, prompt) in filters {
                        let mut input = commands.spawn((
                            TextInputNode {
                                mode: TextInputMode::SingleLine,
                                max_chars: Some(20),
                                ..Default::default()
                            },
                            TextFont {
                                font: assets.load("fonts/FiraMono-Medium.ttf"),
                                font_size: 25.,
                                ..Default::default()
                            },
                            TextInputPrompt::new(prompt),
                            TextColor(LIGHT_GOLDENROD_YELLOW.into()),
                            Node {
                                width: Val::Px(250.),
                                height: Val::Px(30.),
                                ..default()
                            },
                            BackgroundColor(Color::BLACK),
                            Outline {
                                width: Val::Px(2.),
                                offset: Val::Px(2.),
                                color: GREY.into(),
                            },
                        ));
                        if let Some(filter) = filter {
                            input.insert(filter);
                        }

                        let input_entity = input.id();

                        let output_entity = commands.spawn(Text::default()).id();

                        map.insert(input_entity, output_entity);
                    }
                });
        });
    commands.insert_resource(map);
}

fn update(
    input_focus: Res<InputFocus>,
    mut events: MessageReader<SubmitText>,
    map: Res<InputMap>,
    mut text_query: Query<&mut Text>,
    mut outline_query: Query<(Entity, &mut Outline)>,
) {
    if input_focus.is_changed() {
        for (entity, mut outline) in outline_query.iter_mut() {
            if input_focus.0.is_some_and(|active| active == entity) {
                outline.color = Color::WHITE;
            } else {
                outline.color = GREY.into();
            }
        }
    }

    for event in events.read() {
        let out = map[&event.entity];
        text_query.get_mut(out).unwrap().0 = event.text.clone();
    }
}
