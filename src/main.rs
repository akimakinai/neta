use bevy::{
    asset::UnapprovedPathMode,
    dev_tools::picking_debug::{DebugPickingMode, DebugPickingPlugin},
    prelude::*,
    render::pipelined_rendering::PipelinedRenderingPlugin,
    window::PresentMode,
    winit::WinitSettings,
};

mod canvas;
mod error;
mod observe_component;
mod sprite_picking;
mod ui;
mod viewport_delta;

fn main() {
    App::new()
        .insert_resource(WinitSettings::desktop_app())
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                })
                .disable::<PipelinedRenderingPlugin>(),
        )
        // replace with fixed version (https://github.com/bevyengine/bevy/pull/18069)
        .add_plugins(sprite_picking::SpritePickingPlugin)
        .add_plugins(DebugPickingPlugin)
        .insert_resource(DebugPickingMode::Normal)
        .add_plugins((canvas::CanvasPlugin, ui::UiPlugin))
        .run();
}
