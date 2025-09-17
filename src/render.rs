use crate::TextInputBuffer;
use crate::TextInputGlyph;
use crate::TextInputLayoutInfo;
use crate::TextInputNode;
use crate::TextInputPrompt;
use crate::TextInputPromptLayoutInfo;
use crate::TextInputStyle;
use crate::edit::is_buffer_empty;
use bevy::asset::AssetId;
use bevy::asset::Assets;
use bevy::camera::visibility::InheritedVisibility;
use bevy::color::Alpha;
use bevy::color::LinearRgba;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::Commands;
use bevy::ecs::system::Query;
use bevy::ecs::system::Res;
use bevy::ecs::system::ResMut;
use bevy::image::TextureAtlasLayout;
use bevy::input_focus::InputFocus;
use bevy::math::Affine2;
use bevy::math::Rect;
use bevy::math::Vec2;
use bevy::render::Extract;
use bevy::render::sync_world::TemporaryRenderEntity;
use bevy::sprite::BorderRect;
use bevy::text::TextColor;
use bevy::ui::CalculatedClip;
use bevy::ui::ComputedNode;
use bevy::ui::ComputedUiTargetCamera;
use bevy::ui::ResolvedBorderRadius;
use bevy::ui::UiGlobalTransform;
use bevy::ui_render::ExtractedGlyph;
use bevy::ui_render::ExtractedUiItem;
use bevy::ui_render::ExtractedUiNode;
use bevy::ui_render::ExtractedUiNodes;
use bevy::ui_render::NodeType;
use bevy::ui_render::UiCameraMap;
use bevy::ui_render::stack_z_offsets;
use cosmic_text::Edit;

pub fn extract_text_input_nodes(
    mut commands: Commands,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    texture_atlases: Extract<Res<Assets<TextureAtlasLayout>>>,
    active_text_input: Extract<Res<InputFocus>>,
    uinode_query: Extract<
        Query<(
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&CalculatedClip>,
            &ComputedUiTargetCamera,
            &TextInputLayoutInfo,
            &TextColor,
            &TextInputStyle,
            &TextInputNode,
            &TextInputBuffer,
        )>,
    >,
    camera_map: Extract<UiCameraMap>,
) {
    let mut camera_mapper = camera_map.get_mapper();

    let mut start = extracted_uinodes.glyphs.len();
    let mut end = start + 1;

    for (
        entity,
        uinode,
        global_transform,
        inherited_visibility,
        clip,
        target,
        text_layout_info,
        text_color,
        style,
        input,
        input_buffer,
    ) in &uinode_query
    {
        // Skip if not visible or if size is set to zero (e.g. when a parent is set to `Display::None`)
        if !inherited_visibility.get() || uinode.is_empty() {
            continue;
        }

        let Some(extracted_camera_entity) = camera_mapper.map(target) else {
            continue;
        };

        let color = text_color.0.to_linear();
        let selection_color = style
            .selected_text_color
            .map(|selection_color| selection_color.to_linear())
            .unwrap_or(color);

        let scroll = input_buffer
            .editor
            .with_buffer(|buffer| Vec2::new(buffer.scroll().horizontal, 0.)); // buffer.scroll().vertical));

        let transform = Affine2::from(global_transform)
            * Affine2::from_translation(uinode.size() * -0.5 - scroll);

        let node_rect = Rect::from_center_size(
            global_transform.translation,
            uinode.size()
                * Vec2::new(
                    global_transform.matrix2.col(0).length(),
                    global_transform.matrix2.col(1).length(),
                ),
        );

        let clip = Some(
            clip.map(|clip| clip.clip.intersect(node_rect))
                .unwrap_or(node_rect),
        );

        let line_height = input_buffer
            .editor
            .with_buffer(|buffer| buffer.metrics().line_height);

        for (i, rect) in input_buffer.selection_rects.iter().enumerate() {
            let size = if (1..input_buffer.selection_rects.len()).contains(&i) {
                rect.size() + Vec2::Y
            } else {
                rect.size()
            } + 2. * Vec2::X;
            extracted_uinodes.uinodes.push(ExtractedUiNode {
                z_order: uinode.stack_index as f32 + stack_z_offsets::TEXT,
                image: AssetId::default(),
                clip,
                extracted_camera_entity,
                transform: transform * Affine2::from_translation(rect.center()),
                item: ExtractedUiItem::Node {
                    color: LinearRgba::from(style.selection_color),
                    atlas_scaling: None,
                    flip_x: false,
                    flip_y: false,
                    border_radius: ResolvedBorderRadius::ZERO,
                    border: BorderRect::ZERO,
                    node_type: NodeType::Rect,
                    rect: Rect {
                        min: Vec2::ZERO,
                        max: size,
                    },
                },
                main_entity: entity.into(),
                render_entity: commands.spawn(TemporaryRenderEntity).id(),
            });
        }

        let cursor_visable = active_text_input.0.is_some_and(|active| active == entity)
            && input.is_enabled
            && input_buffer.cursor_blink_time < style.blink_interval
            && !style.cursor_color.is_fully_transparent();

        let cursor_position = input_buffer
            .editor
            .cursor_position()
            .filter(|_| cursor_visable);

        let selection = input_buffer.editor.selection_bounds();

        for TextInputGlyph {
            position,
            atlas_info,

            line_index,
            byte_index,
            ..
        } in text_layout_info.glyphs.iter()
        {
            let color_out = if let Some((s0, s1)) = selection {
                if (s0.line < *line_index || (*line_index == s0.line && s0.index <= *byte_index))
                    && (*line_index < s1.line || (*line_index == s1.line && *byte_index < s1.index))
                {
                    selection_color
                } else {
                    color
                }
            } else {
                color
            };

            let Some(rect) = texture_atlases
                .get(atlas_info.texture_atlas)
                .map(|atlas| atlas.textures[atlas_info.location.glyph_index].as_rect())
            else {
                continue;
            };

            extracted_uinodes.glyphs.push(ExtractedGlyph {
                color: color_out,
                translation: *position,
                rect,
            });

            extracted_uinodes.uinodes.push(ExtractedUiNode {
                z_order: uinode.stack_index as f32 + stack_z_offsets::TEXT,
                image: atlas_info.texture,
                clip,
                extracted_camera_entity,
                item: ExtractedUiItem::Glyphs { range: start..end },
                main_entity: entity.into(),
                render_entity: commands.spawn(TemporaryRenderEntity).id(),
                transform,
            });

            start = end;
            end += 1;
        }

        if let Some((x, y)) = cursor_position {
            let cursor_height = line_height * style.cursor_height;

            let x = x as f32;
            let y = y as f32;

            let scale_factor = uinode.inverse_scale_factor().recip();
            let width = style.cursor_width * scale_factor;

            extracted_uinodes.uinodes.push(ExtractedUiNode {
                z_order: uinode.stack_index as f32 + stack_z_offsets::TEXT,
                image: AssetId::default(),
                clip,
                extracted_camera_entity,
                transform: transform
                    * Affine2::from_translation(Vec2::new(x + 0.5 * width, y + 0.5 * line_height)),
                item: ExtractedUiItem::Node {
                    color,
                    atlas_scaling: None,
                    flip_x: false,
                    flip_y: false,
                    border_radius: ResolvedBorderRadius::ZERO,
                    border: BorderRect::ZERO,
                    node_type: NodeType::Rect,
                    rect: Rect {
                        min: Vec2::ZERO,
                        max: Vec2::new(width, cursor_height),
                    },
                },
                main_entity: entity.into(),
                render_entity: commands.spawn(TemporaryRenderEntity).id(),
            });
        }
    }
}

pub fn extract_text_input_prompts(
    mut commands: Commands,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    texture_atlases: Extract<Res<Assets<TextureAtlasLayout>>>,
    uinode_query: Extract<
        Query<(
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&CalculatedClip>,
            &ComputedUiTargetCamera,
            &TextInputPromptLayoutInfo,
            &TextColor,
            &TextInputBuffer,
            &TextInputPrompt,
        )>,
    >,
    camera_map: Extract<UiCameraMap>,
) {
    let mut camera_mapper = camera_map.get_mapper();

    let mut start = extracted_uinodes.glyphs.len();
    let mut end = start + 1;

    for (
        entity,
        uinode,
        global_transform,
        inherited_visibility,
        clip,
        target,
        text_layout_info,
        text_color,
        input,
        prompt,
    ) in &uinode_query
    {
        // only display the prompt if the text input is empty, including whitespace
        if !input.editor.with_buffer(is_buffer_empty) {
            continue;
        }

        // Skip if not visible or if size is set to zero (e.g. when a parent is set to `Display::None`)
        if !inherited_visibility.get() || uinode.is_empty() {
            continue;
        }

        let Some(extracted_camera_entity) = camera_mapper.map(target) else {
            continue;
        };

        let color = prompt.color.unwrap_or(text_color.0).to_linear();

        let transform =
            Affine2::from(global_transform) * Affine2::from_translation(-0.5 * uinode.size());

        let node_rect = Rect::from_center_size(
            global_transform.translation,
            uinode.size()
                * Vec2::new(
                    global_transform.matrix2.col(0).length(),
                    global_transform.matrix2.col(1).length(),
                ),
        );

        let clip = Some(
            clip.map(|clip| clip.clip.intersect(node_rect))
                .unwrap_or(node_rect),
        );

        for TextInputGlyph {
            position,
            atlas_info,
            ..
        } in text_layout_info.glyphs.iter()
        {
            let rect = texture_atlases
                .get(atlas_info.texture_atlas)
                .unwrap()
                .textures[atlas_info.location.glyph_index]
                .as_rect();
            extracted_uinodes.glyphs.push(ExtractedGlyph {
                color,
                translation: *position,
                rect,
            });
            extracted_uinodes.uinodes.push(ExtractedUiNode {
                z_order: uinode.stack_index() as f32 + stack_z_offsets::TEXT,
                transform,
                image: atlas_info.texture,
                clip,
                item: ExtractedUiItem::Glyphs { range: start..end },
                main_entity: entity.into(),
                render_entity: commands.spawn(TemporaryRenderEntity).id(),
                extracted_camera_entity,
            });

            start = end;
            end += 1;
        }
    }
}
