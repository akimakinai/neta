use bevy::{input::common_conditions::input_just_released, prelude::*, window::PrimaryWindow};
use bevy_inspector_egui::{
    bevy_egui::{EguiContext, EguiContextPass, EguiPlugin},
    bevy_inspector::Filter,
    egui,
};

pub fn plugin(app: &mut App) {
    app.add_plugins(EguiPlugin {
        enable_multipass_for_primary_context: true,
    })
    .add_plugins(bevy_inspector_egui::DefaultInspectorConfigPlugin)
    .add_systems(
        EguiContextPass,
        (
            add_inspector_ui.run_if(input_just_released(KeyCode::F12)),
            inspector_ui,
        ),
    );
}

#[derive(Component)]
struct InspectorUi {
    id: u32,
}

fn add_inspector_ui(mut commands: Commands, mut inspector_id: Local<u32>) {
    commands.spawn(InspectorUi { id: *inspector_id });
    *inspector_id += 1;
}

fn inspector_ui(world: &mut World) {
    let Ok(egui_context) = world
        .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
        .single(world)
    else {
        return;
    };
    let mut egui_context = egui_context.clone();

    let ids = world
        .query_filtered::<&InspectorUi, ()>()
        .iter(world)
        .map(|inspector| inspector.id)
        .collect::<Vec<_>>();
    for id in ids {
        egui::Window::new(format!("Inspector {id}")).show(egui_context.get_mut(), |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let filter: Filter = Filter::from_ui_fuzzy(
                    ui,
                    egui::Id::new(format!("default_world_entities_filter_{id}")),
                );
                bevy_inspector_egui::bevy_inspector::ui_for_entities_filtered(
                    world, ui, true, &filter,
                );
            });
        });
    }
}
