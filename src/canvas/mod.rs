use crate::{
    sprite_picking::{SpritePickingMode, SpritePickingSettings},
    viewport_delta::PointerDelta,
};
use bevy::picking::backend::prelude::*;
use bevy::{
    asset::LoadState,
    ecs::schedule::common_conditions,
    prelude::*,
    render::view::RenderLayers,
    window::{PrimaryWindow, RequestRedraw},
};
use bevy_vector_shapes::{
    Shape2dPlugin,
    prelude::ShapePainter,
    shapes::{DiscPainter, LinePainter, RectPainter},
};
use camera_util::CameraTranslator;

mod camera_util;
mod handle;
mod picking;

pub struct CanvasPlugin;

/// Resource to track the state of a rectangular selection drag.
#[derive(Default, Resource)]
struct SelectionDrag {
    start: Option<Vec2>,
    end: Option<Vec2>,
}

impl SelectionDrag {
    fn is_dragging(&self) -> bool {
        self.start.is_some() || self.end.is_some()
    }
}

impl Plugin for CanvasPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpritePickingSettings {
            require_markers: false,
            picking_mode: SpritePickingMode::BoundingBox,
        })
        .insert_resource(SelectionDrag::default())
        .add_plugins(Shape2dPlugin::default())
        .add_plugins(picking::AreaPickingPlugin {
            require_markers: false,
        })
        .add_plugins(handle::ControlHandlePlugin)
        .add_systems(Startup, startup)
        .add_systems(
            Update,
            (
                setup_sprite,
                dummy_paint.run_if(common_conditions::run_once),
            ),
        )
        .add_systems(Update, draw_selection_rectangle)
        .add_systems(
            Update,
            (
                file_drop,
                place_drop_image_frame
                    .run_if(common_conditions::any_with_component::<DropImageFrame>),
            ),
        )
        .add_systems(
            PostUpdate,
            draw_border
                .after(TransformSystem::TransformPropagate)
                .run_if(|current: Option<Res<handle::CurrentControlHandle>>| current.is_none()),
        );
    }
}

#[derive(Component)]
pub struct Canvas;

#[derive(Component)]
struct MainCamera;

/// Camera for control handles.
#[derive(Component)]
struct ControlCamera;

/// Render layer for [`ControlCamera`].
const CONTROL_LAYER: RenderLayers = RenderLayers::layer(1);

fn startup(world: &mut World) {
    world.spawn((Name::new("MainCamera"), Camera2d, MainCamera));

    world.spawn((
        Name::new("ControlCamera"),
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
        CONTROL_LAYER,
        ControlCamera,
    ));

    world.spawn((
        Name::new("Canvas"),
        Canvas,
        Transform::default(),
        Visibility::default(),
    ));

    let primary_window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .unwrap();
    world
        .entity_mut(primary_window)
        .observe(zoom_with_mouse_wheel)
        .observe(drag_with_middle_mouse_button)
        .observe(
            |trigger: Trigger<Pointer<Click>>,
             mut commands: Commands,
             #[cfg(feature = "dev")] egui_wants_input_resource: Res<
                bevy_inspector_egui::bevy_egui::input::EguiWantsInput,
            >| {
                if egui_wants_input_resource.wants_any_input() {
                    return;
                }
                if trigger.event().button == PointerButton::Primary {
                    commands.queue(handle::despawn_control_handle);
                }
            },
        )
        .observe(handle_canvas_click)
        .observe(handle_selection_drag_start)
        .observe(handle_selection_drag)
        .observe(handle_selection_drag_end);
}

fn dummy_paint(mut painter: ShapePainter) {
    // Dummy draw to compile shaders in advance.
    // Missing renders are especially visible with `WinitSettings::desktop_app()`.

    painter.circle(0.0);
    painter.rect(Vec2::splat(0.0));
    painter.line(Vec3::ZERO, Vec3::ZERO);
}

fn zoom_with_mouse_wheel(
    trigger: Trigger<Pointer<Scroll>>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };

    let event = trigger.event();
    if event.y > 0.0 {
        transform.scale *= Vec3::new(1.1, 1.1, 1.0);
    } else {
        transform.scale /= Vec3::new(1.1, 1.1, 1.0);
    }
}

fn drag_with_middle_mouse_button(
    trigger: Trigger<Pointer<Drag>>,
    mut camera: Query<&mut Transform, With<Camera>>,
    pointer_delta: PointerDelta<With<MainCamera>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) {
    if mouse_buttons.any_pressed([MouseButton::Left, MouseButton::Right]) {
        return;
    }

    let event = trigger.event();
    if event.button == PointerButton::Middle {
        if let Some((world_delta, camera_id)) =
            pointer_delta.get_world(&trigger.pointer_location, trigger.delta)
        {
            let Ok(mut transform) = camera.get_mut(camera_id) else {
                return;
            };

            transform.translation -= world_delta.extend(0.0);
        }
    }
}

#[derive(Component)]
pub struct ImageFrame(pub Handle<Image>);

/// Currently hovered frame.
#[derive(Component)]
pub struct Hovered;

/// Currently selected frame.
#[derive(Component, Default)]
pub struct Selected;

fn setup_sprite(
    mut commands: Commands,
    images: Res<Assets<Image>>,
    asset_server: Res<AssetServer>,
    image_frames: Query<(Entity, &ImageFrame, Option<&Transform>), Without<Sprite>>,
    mut index: Local<u32>,
) {
    for (entity, image_frame, orig_transform) in image_frames {
        let Some(image) = images.get(&image_frame.0) else {
            if matches!(
                asset_server.get_load_state(&image_frame.0),
                Some(LoadState::Failed(..)),
            ) {
                commands.entity(entity).despawn();
            }

            continue;
        };
        let size = image.texture_descriptor.size;

        let mut transform = orig_transform.copied().unwrap_or_default();
        transform.translation.z = *index as f32 / 65536.0;

        let id = commands
            .entity(entity)
            .insert((
                Sprite {
                    image: image_frame.0.clone(),
                    custom_size: Some(Vec2::new(size.width as f32, size.height as f32)),
                    ..default()
                },
                transform,
                Pickable::default(),
            ))
            .observe(
                |mut trigger: Trigger<Pointer<Drag>>,
                 mut transform: Query<&mut Transform>,
                 viewport_delta: PointerDelta<With<MainCamera>>| {
                    if trigger.event().button != PointerButton::Primary {
                        return;
                    }

                    trigger.propagate(false);

                    let Ok(mut sprite_tr) = transform.get_mut(trigger.target()) else {
                        return;
                    };

                    if let Some((world_delta, _)) =
                        viewport_delta.get_world(&trigger.pointer_location, trigger.delta)
                    {
                        sprite_tr.translation += world_delta.extend(0.0);
                    }
                },
            )
            .observe(
                |trigger: Trigger<Pointer<Over>>,
                 mut commands: Commands,
                 selection_drag: Res<SelectionDrag>| {
                    if selection_drag.is_dragging() {
                        return;
                    }
                    commands.entity(trigger.target()).insert(Hovered);
                },
            )
            .observe(|trigger: Trigger<Pointer<Out>>, mut commands: Commands| {
                commands.entity(trigger.target()).remove::<Hovered>();
            })
            .observe(
                |mut trigger: Trigger<Pointer<Click>>,
                 mut commands: Commands,
                 selected_query: Query<Entity, With<Selected>>,
                 keyboard_input: Res<ButtonInput<KeyCode>>| {
                    // Prevent click from propagating to canvas background
                    trigger.propagate(false);

                    let ctrl_pressed =
                        keyboard_input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
                    let target_entity = trigger.target();

                    if ctrl_pressed {
                        // Toggle selection
                        if selected_query.get(target_entity).is_ok() {
                            commands.entity(target_entity).remove::<Selected>();
                        } else {
                            commands.entity(target_entity).insert(Selected);
                        }
                    } else {
                        // Deselect all others and select this one
                        for entity in selected_query.iter() {
                            if entity != target_entity {
                                commands.entity(entity).remove::<Selected>();
                            }
                        }
                        commands.entity(target_entity).insert(Selected);
                        commands.queue(handle::spawn_control_handle(target_entity));
                    }
                },
            )
            .id();
        info!("Spawned sprite with id: {id:?}");

        *index += 1;
    }
}

fn draw_border(
    camera_translator: CameraTranslator,
    query: Query<(&GlobalTransform, &Sprite, AnyOf<(&Hovered, &Selected)>)>,
    mut painter: ShapePainter,
) -> Result {
    painter.render_layers = Some(CONTROL_LAYER);
    painter.hollow = true;
    painter.corner_radii = Vec4::splat(5.0);

    for (transform, sprite, (hovered, selected)) in query.iter() {
        let control_transform = camera_translator.to_control(transform)?;

        let size = sprite.custom_size.unwrap_or(Vec2::new(0.0, 0.0)) * control_transform.scale.xy();
        painter.transform = control_transform.with_scale(Vec3::ONE);

        if selected.is_some() {
            painter.color = Color::srgb(0.0, 1.0, 0.0);
        } else if hovered.is_some() {
            painter.color = Color::srgb(1.0, 1.0, 1.0);
        }

        painter.rect(size);
    }

    Ok(())
}

#[derive(Component)]
struct DropImageFrame(Handle<Image>);

fn file_drop(
    mut commands: Commands,
    mut reader: EventReader<FileDragAndDrop>,
    assets: Res<AssetServer>,
    main_window: Single<Entity, With<PrimaryWindow>>,
    canvas_id: Single<Entity, With<Canvas>>,
) {
    for ev in reader.read() {
        match ev {
            FileDragAndDrop::DroppedFile { window, path_buf } => {
                if *window != *main_window {
                    continue;
                }

                // `Window::cursor_position` would return `None` at this point, so we need to spawn the frame
                // after we get the cursor position.

                let img: Handle<Image> = assets.load(path_buf.clone());
                commands.entity(*canvas_id).with_child(DropImageFrame(img));

                commands.send_event(RequestRedraw);
            }
            FileDragAndDrop::HoveredFile { .. } => {}
            FileDragAndDrop::HoveredFileCanceled { .. } => {}
        }
    }
}

fn place_drop_image_frame(
    mut commands: Commands,
    main_window: Single<&Window, With<PrimaryWindow>>,
    main_camera: Single<(&Camera, &GlobalTransform), With<ControlCamera>>,
    image_frames: Query<(Entity, &DropImageFrame, &ChildOf)>,
) {
    let Some(cursor_position) = main_window.cursor_position() else {
        return;
    };

    let Ok(world_position) = main_camera
        .0
        .viewport_to_world_2d(main_camera.1, cursor_position)
    else {
        return;
    };

    for (entity, image_frame, child_of) in image_frames {
        commands.entity(child_of.parent()).with_child((
            ImageFrame(image_frame.0.clone()),
            Transform::from_translation(world_position.extend(0.0)),
        ));
        commands.entity(entity).despawn();
    }
}

/// System to handle the start of a selection drag on the canvas background.
fn handle_selection_drag_start(
    trigger: Trigger<Pointer<DragStart>>,
    mut drag_state: ResMut<SelectionDrag>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }

    drag_state.start = Some(trigger.pointer_location.position);
}

/// System to handle the ongoing selection drag.
fn handle_selection_drag(trigger: Trigger<Pointer<Drag>>, mut drag_state: ResMut<SelectionDrag>) {
    if drag_state.start.is_none() {
        return;
    }
    if trigger.event().button != PointerButton::Primary {
        return;
    }

    drag_state.end = Some(trigger.pointer_location.position);
}

/// System to handle the end of a selection drag.
fn handle_selection_drag_end(
    trigger: Trigger<Pointer<DragEnd>>,
    mut commands: Commands,
    mut drag_state: ResMut<SelectionDrag>,
    image_frames: Query<(Entity, &GlobalTransform, &Sprite), With<ImageFrame>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    selected_query: Query<Entity, With<Selected>>,
    control_camera: Single<(&Camera, &GlobalTransform), With<ControlCamera>>,
) -> Result {
    let (Some(start), Some(end)) = (drag_state.start.take(), drag_state.end.take()) else {
        return Ok(());
    };
    if trigger.event().button != PointerButton::Primary {
        return Ok(());
    }

    let ctrl_pressed = keyboard_input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);

    if !ctrl_pressed {
        // Deselect all if Ctrl is not held
        for entity in selected_query.iter() {
            commands.entity(entity).remove::<Selected>();
        }
    }

    let selection_rect = Rect::from_corners(
        control_camera
            .0
            .viewport_to_world_2d(control_camera.1, start)?,
        control_camera
            .0
            .viewport_to_world_2d(control_camera.1, end)?,
    );

    for (entity, transform, sprite) in image_frames.iter() {
        let sprite_size = sprite.custom_size.unwrap_or(Vec2::ZERO);
        let sprite_rect = Rect::from_center_size(
            transform.translation().xy(),
            sprite_size * transform.scale().xy(),
        );

        if !selection_rect.intersect(sprite_rect).is_empty() {
            commands.entity(entity).insert(Selected);
        }
    }

    Ok(())
}

/// System to draw the selection rectangle.
fn draw_selection_rectangle(
    drag_state: Res<SelectionDrag>,
    mut painter: ShapePainter,
    control_camera: Single<(&Camera, &GlobalTransform), With<ControlCamera>>,
) -> Result {
    let (Some(start), Some(end)) = (drag_state.start, drag_state.end) else {
        return Ok(());
    };

    let start = control_camera
        .0
        .viewport_to_world_2d(control_camera.1, start)?;
    let end = control_camera
        .0
        .viewport_to_world_2d(control_camera.1, end)?;

    let selection_rect = Rect::from_corners(start, end);

    painter.render_layers = Some(CONTROL_LAYER);
    painter.hollow = true;
    painter.color = Color::srgba(0.5, 0.5, 1.0, 0.5);
    painter.transform = Transform::from_translation(selection_rect.center().extend(0.0));
    painter.rect(selection_rect.size());

    Ok(())
}

/// System to handle clicks on the canvas background for deselection.
fn handle_canvas_click(
    trigger: Trigger<Pointer<Click>>,
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    selected_query: Query<Entity, With<Selected>>,
) {
    if trigger.button != PointerButton::Primary {
        return;
    }

    let ctrl_pressed = keyboard_input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);

    if ctrl_pressed {
        return;
    }

    // Deselect all if clicking on canvas background without Ctrl
    for entity in selected_query.iter() {
        commands.entity(entity).remove::<Selected>();
    }
}
