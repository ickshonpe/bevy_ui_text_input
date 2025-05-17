pub mod clipboard;
pub mod edit;
pub mod render;
pub mod text_input_pipeline;

use bevy::app::{Plugin, PostUpdate};
use bevy::asset::AssetEvents;
use bevy::color::Color;
use bevy::color::palettes::css::SKY_BLUE;
use bevy::color::palettes::tailwind::GRAY_400;
use bevy::ecs::component::{Component, HookContext};
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::observer::Observer;
use bevy::ecs::query::Changed;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::resource_changed;
use bevy::ecs::system::Query;
use bevy::ecs::world::DeferredWorld;
use bevy::input_focus::InputFocus;
use bevy::math::{Rect, Vec2};
use bevy::prelude::ReflectComponent;
use bevy::reflect::{Reflect, std_traits::ReflectDefault};
use bevy::render::{ExtractSchedule, RenderApp};
use bevy::text::TextColor;
use bevy::text::cosmic_text::{Align, Buffer, Change, Edit, Editor, Metrics, Wrap};
use bevy::text::{GlyphAtlasInfo, TextFont};
use bevy::ui::{Node, RenderUiSystem, UiSystem, extract_text_sections};
use edit::{
    clear_selection_on_focus_change, mouse_wheel_scroll, on_drag_text_input,
    on_move_clear_multi_click, on_multi_click_set_selection, on_text_input_pressed,
    text_input_edit_system,
};
use render::{extract_text_input_nodes, extract_text_input_prompts};
use text_input_pipeline::{
    TextInputPipeline, remove_dropped_font_atlas_sets_from_text_input_pipeline,
    text_input_prompt_system, text_input_system,
};
pub struct TextInputPlugin;

impl Plugin for TextInputPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_event::<TextSubmissionEvent>()
            .add_event::<SubmitTextEvent>()
            .init_resource::<InputFocus>()
            .init_resource::<TextInputPipeline>()
            .init_resource::<clipboard::Clipboard>()
            .add_systems(
                PostUpdate,
                (
                    remove_dropped_font_atlas_sets_from_text_input_pipeline.before(AssetEvents),
                    (
                        mouse_wheel_scroll,
                        text_input_edit_system,
                        update_text_input_contents,
                        text_input_system,
                        text_input_prompt_system,
                        clear_selection_on_focus_change.run_if(resource_changed::<InputFocus>),
                    )
                        .chain()
                        .in_set(UiSystem::PostLayout),
                ),
            );

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.add_systems(
            ExtractSchedule,
            (extract_text_input_prompts, extract_text_input_nodes)
                .chain()
                .in_set(RenderUiSystem::ExtractText)
                .after(extract_text_sections),
        );
    }
}

#[derive(Component, Debug)]
#[require(
    Node,
    TextInputBuffer,
    TextFont,
    TextInputLayoutInfo,
    TextInputStyle,
    TextColor
)]
#[component(
    on_add = on_add_textinputnode,
    on_remove = on_remove_unfocus,
)]
pub struct TextInputNode {
    /// Whether the text should be cleared on submission
    /// (Shift-Enter or just Enter in single-line mode)
    pub clear_on_submit: bool,
    /// Type of text input
    pub mode: TextInputMode,
    /// Maximum number of characters that can entered into the input buffer
    pub max_chars: Option<usize>,
    /// Should overwrite mode be available
    pub allow_overwrite_mode: bool,
    /// Can the text input be activated
    pub is_enabled: bool,
    /// Activate on pointer down
    pub focus_on_pointer_down: bool,
    /// Deactivate after text submitted
    pub unfocus_on_submit: bool,
    /// Text alignment
    pub alignment: Option<Align>,
}

impl Default for TextInputNode {
    fn default() -> Self {
        Self {
            clear_on_submit: true,
            mode: TextInputMode::default(),
            max_chars: None,
            allow_overwrite_mode: true,
            is_enabled: true,
            focus_on_pointer_down: true,
            unfocus_on_submit: true,
            alignment: None,
        }
    }
}

fn on_add_textinputnode(mut world: DeferredWorld, context: HookContext) {
    for mut observer in [
        Observer::new(on_drag_text_input),
        Observer::new(on_text_input_pressed),
        Observer::new(on_multi_click_set_selection),
        Observer::new(on_move_clear_multi_click),
    ] {
        observer.watch_entity(context.entity);
        world.commands().spawn(observer);
    }
}

fn on_remove_unfocus(mut world: DeferredWorld, context: HookContext) {
    let mut input_focus = world.resource_mut::<InputFocus>();
    if input_focus.0 == Some(context.entity) {
        input_focus.0 = None;
    }
}

/// Sent when a text input submits its text
#[derive(Event)]
pub struct TextSubmissionEvent {
    /// The text input entity that submitted the text
    pub entity: Entity,
    /// The submitted text
    pub text: String,
}

/// Send to submit the text input entity's text
#[derive(Event)]
pub struct SubmitTextEvent {
    /// The text input entity that should submit its text
    pub entity: Entity,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TextInputMode {
    /// Scrolling text input
    /// Submit on shift-enter
    Text { wrap: Wrap },
    /// Single line numeric input
    /// Scrolls horizontally
    /// Submit on enter
    Integer,
    /// Single line decimal input
    /// Scrolls horizontally
    /// Submit on enter
    Decimal,
    /// Single line hexadecimal input
    /// Scrolls horizontally
    /// Submit on enter
    Hex,
    /// Single line text input
    /// Scrolls horizontally
    /// Submit on enter
    TextSingleLine,
}

impl Default for TextInputMode {
    fn default() -> Self {
        Self::Text {
            wrap: Wrap::WordOrGlyph,
        }
    }
}

impl TextInputMode {
    pub fn wrap(&self) -> Wrap {
        match self {
            TextInputMode::Text { wrap } => *wrap,
            _ => Wrap::None,
        }
    }
}

#[derive(Component, Debug)]
pub struct TextInputBuffer {
    set_text: Option<String>,
    pub editor: Editor<'static>,
    pub(crate) selection_rects: Vec<Rect>,
    pub(crate) cursor_blink_time: f32,
    pub(crate) overwrite_mode: bool,
    pub(crate) needs_update: bool,
    pub(crate) prompt_buffer: Option<Buffer>,
    pub(crate) changes: cosmic_undo_2::Commands<Change>,
}

impl TextInputBuffer {
    /// set the text for the input, overwriting any existing contents.
    pub fn set_text(&mut self, text: String) {
        self.set_text = Some(text);
    }

    /// clear the input
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }

    pub fn get_text(&self) -> String {
        self.editor.with_buffer(get_text)
    }
}

impl Default for TextInputBuffer {
    fn default() -> Self {
        Self {
            set_text: None,
            editor: Editor::new(Buffer::new_empty(Metrics::new(20.0, 20.0))),
            selection_rects: vec![],
            cursor_blink_time: 0.,
            overwrite_mode: false,
            needs_update: true,
            prompt_buffer: None,
            changes: cosmic_undo_2::Commands::default(),
        }
    }
}

/// Prompt displayed when the input is empty (including whitespace).
/// Optional component.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component, Default, Debug)]
#[require(TextInputPromptLayoutInfo)]
pub struct TextInputPrompt {
    /// Prompt's text
    pub text: String,
    /// The prompt's font.
    /// If none, the text input's font is used.
    pub font: Option<TextFont>,
    /// The color of the prompt's text.
    /// If none, the text input's `TextColor` is used.
    pub color: Option<Color>,
}

impl TextInputPrompt {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }
}

impl Default for TextInputPrompt {
    fn default() -> Self {
        Self {
            text: "Enter some text here".into(),
            font: None,
            color: Some(bevy::color::palettes::css::GRAY.into()),
        }
    }
}

/// Styling for a text cursor
#[derive(Component, Copy, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component, Default, Debug, PartialEq)]
pub struct TextInputStyle {
    /// Color of the cursor
    pub cursor_color: Color,
    /// Selection color
    pub selection_color: Color,
    /// Selected text tint, if unset uses the `TextColor`
    pub selected_text_color: Option<Color>,
    /// Width of the cursor
    pub cursor_width: f32,
    /// Corner radius in logical pixels
    pub cursor_radius: f32,
    /// Normalized height of the cursor relative to the text block's line height.
    pub cursor_height: f32,
    /// Time cursor blinks in seconds
    pub blink_interval: f32,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            cursor_color: GRAY_400.into(),
            selection_color: SKY_BLUE.into(),
            selected_text_color: None,
            cursor_width: 3.,
            cursor_radius: 0.,
            cursor_height: 1.,
            blink_interval: 0.5,
        }
    }
}

fn get_text(buffer: &Buffer) -> String {
    buffer
        .lines
        .iter()
        .map(|buffer_line| buffer_line.text())
        .fold(String::new(), |mut out, line| {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
            out
        })
}

#[derive(Component, Clone, Default, Debug, Reflect)]
#[reflect(Component, Default, Debug)]
pub struct TextInputLayoutInfo {
    pub glyphs: Vec<TextInputGlyph>,
    pub size: Vec2,
}

#[derive(Component, Clone, Default, Debug, Reflect)]
#[reflect(Component, Default, Debug)]
pub struct TextInputPromptLayoutInfo {
    pub glyphs: Vec<TextInputGlyph>,
    pub size: Vec2,
}

#[derive(Debug, Clone, Reflect)]
pub struct TextInputGlyph {
    pub position: Vec2,
    pub size: Vec2,
    pub atlas_info: GlyphAtlasInfo,
    pub span_index: usize,
    pub line_index: usize,
    pub byte_index: usize,
    pub byte_length: usize,
}

#[derive(Default, Debug, Component, PartialEq)]
pub struct TextInputContents {
    text: String,
}

impl TextInputContents {
    pub fn get(&self) -> &str {
        &self.text
    }
}

pub fn update_text_input_contents(
    mut query: Query<(&TextInputBuffer, &mut TextInputContents), Changed<TextInputBuffer>>,
) {
    for (buffer, mut contents) in query.iter_mut() {
        let text = buffer.get_text();
        if contents.text != text {
            contents.text = text;
        }
    }
}
