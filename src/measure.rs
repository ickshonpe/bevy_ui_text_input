use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::Commands;
use bevy::ecs::system::Query;
use bevy::ecs::world::Ref;
use bevy::math::Vec2;
use bevy::text::TextFont;
use bevy::ui::ComputedUiRenderTargetInfo;
use bevy::ui::ContentSize;
use bevy::ui::Measure;
use bevy::ui::MeasureArgs;
use bevy::ui::NodeMeasure;
use taffy::MaybeMath;
use taffy::MaybeResolve;
use taffy::Style;

use crate::TextInputNode;

/// Measure that automatically sets a Text Input's height
struct InputMeasure {
    // height in target pixels
    height: f32,
}

impl Measure for InputMeasure {
    fn measure(&mut self, measure_args: MeasureArgs, style: &Style) -> Vec2 {
        let parent_width = measure_args.available_width.into_option();
        let s_width = style.size.width.maybe_resolve(parent_width);
        let s_min_width = style.min_size.width.maybe_resolve(parent_width);
        let s_max_width = style.max_size.width.maybe_resolve(parent_width);
        let width = measure_args
            .width
            .or(s_width)
            .or(s_min_width)
            .maybe_clamp(s_min_width, s_max_width);

        let size = taffy::Size {
            width,
            height: Some(self.height),
        }
        .maybe_apply_aspect_ratio(style.aspect_ratio);

        Vec2::new(
            size.width
                .or(parent_width)
                .maybe_clamp(s_min_width, s_max_width)
                .unwrap_or(0.),
            self.height,
        )
    }
}

pub fn measure_lines(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        Ref<ComputedUiRenderTargetInfo>,
        Ref<TextFont>,
        Ref<TextInputNode>,
    )>,
) {
    for (entity, target, text_font, text_node) in query.iter_mut() {
        if target.is_changed() || text_font.is_changed() || text_node.is_changed() {
            let lines = match text_node.mode {
                crate::TextInputMode::MultiLine { lines, .. } => {
                    if lines <= 0. {
                        commands.entity(entity).remove::<ContentSize>();
                        continue;
                    }
                    lines
                }
                crate::TextInputMode::SingleLine => 1.,
            };

            let line_height = match text_font.line_height {
                bevy::text::LineHeight::Px(px) => px,
                bevy::text::LineHeight::RelativeToFont(r) => r * text_font.font_size,
            } * target.scale_factor();
            let height = lines * line_height;
            let mut measure = ContentSize::default();
            measure.set(NodeMeasure::Custom(Box::new(InputMeasure { height })));
            commands.entity(entity).insert(measure);
        }
    }
}
