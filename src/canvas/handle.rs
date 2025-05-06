use bevy::{
    color::palettes::css::*,
    prelude::*,
    window::{PrimaryWindow, SystemCursorIcon},
    winit::cursor::CursorIcon,
};
use bevy_vector_shapes::{
    prelude::ShapePainter,
    shapes::{DiscPainter, LinePainter, RectPainter},
};

use crate::{observe_component::Observe, viewport_delta::PointerDelta};

use super::{CONTROL_LAYER, MainCamera, camera_util::CameraTranslator, picking::PickingAreaCircle};

pub struct ControlHandlePlugin;

impl Plugin for ControlHandlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                track_main_camera_entity_transform,
                (update_corner_handle, update_rotation_handle),
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            draw_control_handle.after(TransformSystem::TransformPropagate),
        );
    }
}

#[derive(Component)]
#[relationship(relationship_target = ControlledSprite)]
pub struct ControlHandle(#[relationship] pub Entity);

#[derive(Component)]
#[relationship_target(relationship = ControlHandle)]
pub struct ControlledSprite(Entity);

#[derive(Resource, Debug)]
pub struct CurrentControlHandle(pub Entity);

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Pivot {
    BottomLeft,
    BottomCenter,
    BottomRight,
    CenterLeft,
    CenterRight,
    TopLeft,
    TopCenter,
    TopRight,
}

impl Pivot {
    pub fn as_vec(&self) -> Vec2 {
        match self {
            Pivot::BottomLeft => Vec2::new(-0.5, -0.5),
            Pivot::BottomCenter => Vec2::new(0.0, -0.5),
            Pivot::BottomRight => Vec2::new(0.5, -0.5),
            Pivot::CenterLeft => Vec2::new(-0.5, 0.0),
            Pivot::CenterRight => Vec2::new(0.5, 0.0),
            Pivot::TopLeft => Vec2::new(-0.5, 0.5),
            Pivot::TopCenter => Vec2::new(0.0, 0.5),
            Pivot::TopRight => Vec2::new(0.5, 0.5),
        }
    }
}

#[derive(Component)]
struct ControlHandleCorner(Pivot);

#[derive(Component)]
struct ControlHandleRotation(Pivot);

const CORNER_HANDLE_RADIUS: f32 = 10.0;

pub fn spawn_control_handle(sprite_id: Entity) -> impl Command<Result> {
    move |world: &mut World| -> Result {
        if let Some(current_handle) = world.get_resource::<CurrentControlHandle>() {
            world.entity_mut(current_handle.0).despawn();
        }

        let mut commands = world.commands();

        let mut handle = commands.spawn((
            Name::new("ControlHandle"),
            ControlHandle(sprite_id),
            Transform::default(),
            Visibility::default(),
            TrackMainCameraEntityTransform(sprite_id),
            CONTROL_LAYER,
        ));
        handle.with_children(|parent| {
            for pivot in [
                Pivot::TopLeft,
                Pivot::TopRight,
                Pivot::BottomLeft,
                Pivot::BottomRight,
                // Pivot::TopCenter,
                // Pivot::CenterLeft,
                // Pivot::CenterRight,
                // Pivot::BottomCenter,
            ] {
                parent.spawn((
                    CONTROL_LAYER,
                    PickingAreaCircle(Circle::new(CORNER_HANDLE_RADIUS)),
                    ControlHandleCorner(pivot),
                    Transform::from_translation(Vec3::new(0., 0., 2.)),
                    drag_handle_observers(pivot, sprite_id),
                ));
            }

            parent.spawn((
                CONTROL_LAYER,
                PickingAreaCircle(Circle::new(CORNER_HANDLE_RADIUS)),
                ControlHandleRotation(Pivot::TopCenter),
                Transform::from_translation(Vec3::new(0., 100., 2.)),
                rotation_handle_observers(Pivot::TopCenter, sprite_id),
            ));
        });

        let handle = handle.id();
        commands.insert_resource(CurrentControlHandle(handle));

        Ok(())
    }
}

#[derive(Component)]
struct TrackMainCameraEntityTransform(Entity);

fn track_main_camera_entity_transform(
    mut commands: Commands,
    transform_helper: TransformHelper,
    camera_translator: CameraTranslator,
    tracker: Query<(Entity, &TrackMainCameraEntityTransform)>,
) -> Result {
    for (tracker_entity, target) in &tracker {
        let target_entity = target.0;
        let target_transform = transform_helper.compute_global_transform(target_entity)?;

        let control_transform = camera_translator.to_control(&target_transform)?;

        commands.queue(move |world: &mut World| {
            world
                .get_mut::<Transform>(tracker_entity)
                .map(|mut t| t.set_if_neq(control_transform));
        });
    }

    Ok(())
}

fn drag_handle_observers(pivot: Pivot, sprite_id: Entity) -> impl Bundle {
    let cursor_icon = anchor_to_cursor_icon(pivot);

    (
        Observe::new(
            move |mut trigger: Trigger<Pointer<Drag>>,
                  viewport_delta: PointerDelta<With<MainCamera>>,
                  mut sprites: Query<(&mut Transform, &mut Sprite)>| {
                trigger.propagate(false);

                let Some((delta, _)) =
                    viewport_delta.get_world(&trigger.pointer_location, trigger.delta)
                else {
                    return;
                };

                let Ok((mut transform, mut sprite)) = sprites.get_mut(sprite_id) else {
                    return;
                };

                // Resize with the opposite corner being fixed

                transform.translation += delta.extend(0.0) / 2.0;

                let sign = match pivot {
                    Pivot::TopLeft => Vec2::new(-1., 1.),
                    Pivot::TopRight => Vec2::new(1., 1.),
                    Pivot::BottomLeft => Vec2::new(-1., -1.),
                    Pivot::BottomRight => Vec2::new(1., -1.),
                    _ => {
                        return;
                    }
                };

                let rotated_delta = transform
                    .rotation
                    .inverse()
                    .mul_vec3(delta.extend(0.))
                    .truncate()
                    * sign;

                if let Some(custom_size) = sprite.custom_size.as_mut() {
                    *custom_size += rotated_delta;
                } else {
                    error_once!("Sprite is missing custom size");
                }
            },
        ),
        Observe::new(
            move |mut trigger: Trigger<Pointer<Over>>,
                  mut commands: Commands,
                  window: Query<Entity, With<Window>>| {
                trigger.propagate(false);
                window.iter().for_each(|window| {
                    commands
                        .entity(window)
                        .insert(CursorIcon::System(cursor_icon));
                });
            },
        ),
        Observe::new(
            |mut trigger: Trigger<Pointer<Out>>,
             mut commands: Commands,
             window: Query<Entity, With<Window>>| {
                trigger.propagate(false);
                window.iter().for_each(|window| {
                    commands.entity(window).remove::<CursorIcon>();
                });
            },
        ),
        Observe::new(
            |mut trigger: Trigger<Pointer<DragEnd>>,
             mut commands: Commands,
             window: Query<Entity, With<Window>>| {
                trigger.propagate(false);
                window.iter().for_each(|window| {
                    commands.entity(window).remove::<CursorIcon>();
                });
            },
        ),
        Observe::new(|mut trigger: Trigger<Pointer<Click>>| {
            trigger.propagate(false);
        }),
    )
}

fn rotation_handle_observers(pivot: Pivot, sprite_id: Entity) -> impl Bundle {
    (
        Observe::new(
            move |mut trigger: Trigger<Pointer<Drag>>,
                  main_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
                  primary_window: Query<Entity, With<PrimaryWindow>>,
                  mut transform: Query<&mut Transform>,
                  mut commands: Commands,
                  window: Query<Entity, With<Window>>| {
                trigger.propagate(false);

                window.iter().for_each(|window| {
                    commands
                        .entity(window)
                        .insert(CursorIcon::System(SystemCursorIcon::Grabbing));
                });

                let Ok(mut sprite_transform) = transform.get_mut(sprite_id) else {
                    return;
                };

                let Ok((main_camera, main_camera_transform)) = main_camera.single() else {
                    return;
                };
                let primary_window = primary_window.single().ok();

                if main_camera.target.normalize(primary_window).as_ref()
                    != Some(&trigger.pointer_location.target)
                {
                    error_once!(?trigger, "not targetting MainCamera");
                    return;
                }

                let Ok(cursor_world_pos) = main_camera
                    .viewport_to_world_2d(main_camera_transform, trigger.pointer_location.position)
                else {
                    return;
                };

                let diff = cursor_world_pos - sprite_transform.translation.truncate();
                sprite_transform.rotation =
                    Quat::from_rotation_arc_2d(pivot.as_vec().normalize(), diff.normalize());
            },
        ),
        Observe::new(
            move |mut trigger: Trigger<Pointer<Over>>,
                  mut commands: Commands,
                  window: Query<Entity, With<Window>>| {
                trigger.propagate(false);
                window.iter().for_each(|window| {
                    commands
                        .entity(window)
                        .insert(CursorIcon::System(SystemCursorIcon::Grab));
                });
            },
        ),
        Observe::new(
            |mut trigger: Trigger<Pointer<Out>>,
             mut commands: Commands,
             window: Query<Entity, With<Window>>| {
                trigger.propagate(false);
                window.iter().for_each(|window| {
                    commands.entity(window).remove::<CursorIcon>();
                });
            },
        ),
        Observe::new(
            |mut trigger: Trigger<Pointer<DragEnd>>,
             mut commands: Commands,
             window: Query<Entity, With<Window>>| {
                trigger.propagate(false);
                window.iter().for_each(|window| {
                    commands.entity(window).remove::<CursorIcon>();
                });
            },
        ),
        Observe::new(|mut trigger: Trigger<Pointer<Click>>| {
            trigger.propagate(false);
        }),
    )
}

fn anchor_to_cursor_icon(pivot: Pivot) -> SystemCursorIcon {
    match pivot {
        Pivot::TopLeft => SystemCursorIcon::NwResize,
        Pivot::TopRight => SystemCursorIcon::NeResize,
        Pivot::BottomLeft => SystemCursorIcon::SwResize,
        Pivot::BottomRight => SystemCursorIcon::SeResize,
        _ => SystemCursorIcon::Default,
    }
}

pub fn despawn_control_handle(world: &mut World) {
    let current = world.get_resource::<CurrentControlHandle>();
    if let Some(current_handle) = current {
        world.entity_mut(current_handle.0).despawn();
        world.remove_resource::<CurrentControlHandle>();
    }
}

fn update_corner_handle(
    control_handle: Query<&ControlHandle>,
    child_of: Query<&ChildOf>,
    mut handle: Query<(Entity, &mut Transform, Ref<ControlHandleCorner>), Without<MainCamera>>,
    sprite: Query<Ref<Sprite>>,
    images: Res<Assets<Image>>,
    main_camera: Query<&Transform, With<MainCamera>>,
) -> Result {
    let main_camera_transform = main_camera.single()?;

    for (id, mut transform, pivot) in handle.iter_mut() {
        let sprite_id = control_handle.get(child_of.get(id)?.parent())?.0;

        let sprite = sprite.get(sprite_id)?;

        if !pivot.is_changed() && !sprite.is_changed() {
            continue;
        }

        if let Some(mut size) = sprite
            .custom_size
            .or_else(|| images.get(&sprite.image).map(|img| img.size_f32()))
        {
            size /= main_camera_transform.scale.xy();

            let v = pivot.0.as_vec();
            transform.translation = Vec3::new(size.x * v.x, size.y * v.y, transform.translation.z);
        }
    }

    Ok(())
}

const ROTATION_HANDLE_EXTENSION: f32 = 50.0;

fn update_rotation_handle(
    camera_translator: CameraTranslator,
    control_handle: Query<&ControlHandle>,
    child_of: Query<&ChildOf>,
    mut handle: Query<(Entity, &mut Transform, Ref<ControlHandleRotation>), Without<MainCamera>>,
    sprite: Query<(&GlobalTransform, Ref<Sprite>)>,
    images: Res<Assets<Image>>,
) -> Result {
    for (id, mut transform, pivot) in handle.iter_mut() {
        let sprite_id = control_handle.get(child_of.get(id)?.parent())?.0;

        let (sprite_transform, sprite) = sprite.get(sprite_id)?;

        if !pivot.is_changed() && !sprite.is_changed() {
            continue;
        }

        if let Some(mut size) = sprite
            .custom_size
            .or_else(|| images.get(&sprite.image).map(|img| img.size_f32()))
        {
            size *= camera_translator.to_control(sprite_transform)?.scale.xy();

            let v = pivot.0.as_vec();
            let handle_extention = ROTATION_HANDLE_EXTENSION * v.normalize();
            transform.translation = Vec3::new(size.x * v.x, size.y * v.y, transform.translation.z)
                + handle_extention.extend(0.0);
        }
    }

    Ok(())
}
const HANDLE_WIDTH: f32 = 2.0;

fn draw_control_handle(
    camera_translator: CameraTranslator,
    handle_frames: Query<(&ControlHandle, &Children)>,
    handles: Query<
        (&GlobalTransform, Option<&ControlHandleRotation>),
        Or<(With<ControlHandleCorner>, With<ControlHandleRotation>)>,
    >,
    frame: Query<(&GlobalTransform, &Sprite)>,
    mut painter: ShapePainter,
) -> Result {
    painter.render_layers = Some(CONTROL_LAYER);

    for (handle, children) in handle_frames.iter() {
        let (sprite_transform, sprite) = frame.get(handle.0)?;

        let Some(sprite_size) = sprite.custom_size else {
            return Ok(());
        };

        let control_transform = camera_translator.to_control(sprite_transform)?;

        let frame_size = sprite_size * control_transform.scale.xy();

        painter.transform = control_transform;
        painter.transform.translation.z = 2.0;

        // border
        painter.hollow = true;
        painter.color = Color::WHITE;
        painter.thickness = HANDLE_WIDTH;
        painter.rect(frame_size);

        for (transform, rotation_handle) in handles.iter_many(children) {
            painter.transform.translation = transform.translation().with_z(3.0);
            painter.hollow = false;
            painter.thickness = 0.0;
            painter.color = Color::WHITE;
            painter.circle(CORNER_HANDLE_RADIUS);

            painter.hollow = true;
            painter.color = LIGHT_GRAY.into();
            painter.thickness = 1.0;
            painter.circle(CORNER_HANDLE_RADIUS + painter.thickness / 2.);

            if let Some(rotation_handle) = rotation_handle {
                painter.transform = control_transform;
                painter.color = Color::WHITE;

                let v = rotation_handle.0.as_vec();
                let start = v * frame_size;
                painter.line(
                    start.extend(0.0),
                    (start + v.normalize() * ROTATION_HANDLE_EXTENSION).extend(0.0),
                );
            }
        }
    }

    Ok(())
}
