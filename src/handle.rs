use bevy::{
    ecs::system::RunSystemOnce,
    picking::{
        PickSet,
        backend::{HitData, PointerHits},
        pointer::{PointerId, PointerLocation},
    },
    prelude::*,
    sprite::Anchor,
    window::PrimaryWindow,
};
use bevy_vector_shapes::{
    prelude::ShapePainter,
    shapes::{DiscPainter, RectPainter},
};

pub struct ControlHandlePlugin;

impl Plugin for ControlHandlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, pick_handle.in_set(PickSet::Backend))
            .add_systems(Update, draw_control_handle)
            .add_systems(Update, update_sprite_anchored);
    }
}

#[derive(Component)]
#[require(Transform)]
pub struct ControlHandleRect(pub Rect);

#[derive(Component)]
#[require(Transform)]
pub struct ControlHandleCircle(pub Circle);

#[derive(Component)]
pub struct ControlHandle;

#[derive(Resource)]
pub struct CurrentControlHandle(pub Entity);

#[derive(Component)]
struct SpriteAnchored(Anchor);

pub fn spawn_control_handle(parent: Entity) -> impl Command<Result> {
    move |world: &mut World| -> Result {
        // TODO: cache system
        world.run_system_once(
            move |mut commands: Commands, current: Option<Res<CurrentControlHandle>>| {
                if let Some(current_handle) = current {
                    commands.entity(current_handle.0).despawn();
                    commands.remove_resource::<CurrentControlHandle>();
                }

                let mut handle = commands.spawn((
                    Name::new("ControlHandle"),
                    ControlHandle,
                    Transform::default(),
                    Visibility::default(),
                    ChildOf(parent),
                ));
                handle.with_children(|parent| {
                    for anchor in [
                        Anchor::TopLeft,
                        Anchor::TopRight,
                        Anchor::BottomLeft,
                        Anchor::BottomRight,
                    ] {
                        parent.spawn((
                            ControlHandleCircle(Circle::new(10.0)),
                            SpriteAnchored(anchor),
                        ));
                    }
                });

                let handle = handle.id();
                commands.insert_resource(CurrentControlHandle(handle));
            },
        )?;
        Ok(())
    }
}

fn update_sprite_anchored(
    child_of: Query<&ChildOf>,
    mut handle: Query<(Entity, &mut Transform, &SpriteAnchored)>,
    sprite: Query<&Sprite>,
    images: Res<Assets<Image>>,
) {
    for (id, mut transform, anchor) in handle.iter_mut() {
        let Some(sprite_id) = child_of.iter_ancestors(id).nth(1) else {
            error_once!("ControlHandle has no parent sprite");
            continue;
        };

        let sprite = sprite.get(sprite_id).unwrap();
        if let Some(size) = sprite
            .custom_size
            .or_else(|| images.get(&sprite.image).map(|img| img.size_f32()))
        {
            let v = anchor.0.as_vec();
            transform.translation = Vec3::new(size.x * v.x, size.y * v.y, transform.translation.z);
        }
    }
}

fn draw_control_handle(
    handle: Query<&ChildOf, With<ControlHandle>>,
    frame: Query<(&Sprite, &GlobalTransform)>,
    mut painter: ShapePainter,
) {
    for child_of in handle.iter() {
        let Ok((sprite, frame_transform)) = frame.get(child_of.parent()) else {
            return;
        };
        let Some(sprite_size) = sprite.custom_size else {
            return;
        };
        let frame_size =
            (sprite_size.extend(0.) * frame_transform.compute_transform().scale).truncate();

        painter.transform = frame_transform.compute_transform();
        painter.transform.translation.z = 2.0;

        painter.hollow = true;
        painter.color = Color::WHITE;
        painter.thickness = 5.0;
        painter.rect(frame_size);

        painter.hollow = false;
        painter.thickness = 2.0;

        for point in [
            Vec2::new(-0.5, -0.5),
            Vec2::new(0.5, -0.5),
            Vec2::new(-0.5, 0.5),
            Vec2::new(0.5, 0.5),
        ] {
            painter.transform.translation = frame_transform
                .transform_point((point * sprite_size).extend(0.))
                .with_z(3.0);
            painter.circle(10.);
        }

        // painter.transform.translation.z += 1.0;
        // painter.hollow = true;
        // painter.color = Color::BLACK;
        // painter.circle(11.);
    }
}

fn pick_handle(
    pointers: Query<(&PointerId, &PointerLocation)>,
    cameras: Query<(Entity, &Camera, &GlobalTransform, &Projection)>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    handle_shapes: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&ControlHandleRect>,
            Option<&ControlHandleCircle>,
            Option<&Pickable>,
        ),
        Or<(With<ControlHandleRect>, With<ControlHandleCircle>)>,
    >,
    mut output: EventWriter<PointerHits>,
) {
    // based on bevy_sprite\src\picking_backend.rs

    let mut sorted_handles = handle_shapes
        .iter()
        .filter(|(_, transform, ..)| !transform.affine().is_nan())
        .collect::<Vec<_>>();
    radsort::sort_by_key(&mut sorted_handles, |(_, transform, ..)| {
        -transform.translation().z
    });

    let primary_window = primary_window.single().ok();

    for (pointer, pointer_location) in pointers.iter() {
        let Some(location) = pointer_location.location() else {
            continue;
        };

        let Some((cam_entity, camera, cam_transform, Projection::Orthographic(cam_ortho))) =
            cameras
                .iter()
                .filter(|(_, camera, _, _)| camera.is_active)
                .find(|(_, camera, _, _)| {
                    camera
                        .target
                        .normalize(primary_window)
                        .is_some_and(|x| x == location.target)
                })
        else {
            continue;
        };

        let Ok(cursor_ray_world) = camera.viewport_to_world(cam_transform, location.position) else {
            continue;
        };
        let cursor_ray_len = cam_ortho.far - cam_ortho.near;
        let cursor_ray_end = cursor_ray_world.origin + cursor_ray_world.direction * cursor_ray_len;

        let mut picks = vec![];

        for (entity, handle_transform, rect, circle, pickable) in &sorted_handles {
            // Transform cursor line segment to handle coordinate system
            let world_to_handle = handle_transform.affine().inverse();
            let cursor_start_handle = world_to_handle.transform_point3(cursor_ray_world.origin);
            let cursor_end_handle = world_to_handle.transform_point3(cursor_ray_end);

            // Find where the cursor segment intersects the plane Z=0 (which is the handle's
            // plane in handle-local space). It may not intersect if, for example, we're
            // viewing the handle side-on
            if cursor_start_handle.z == cursor_end_handle.z {
                // Cursor ray is parallel to the handle and misses it
                continue;
            }
            let lerp_factor = f32::inverse_lerp(cursor_start_handle.z, cursor_end_handle.z, 0.0);
            if !(0.0..=1.0).contains(&lerp_factor) {
                // Lerp factor is out of range, meaning that while an infinite line cast by
                // the cursor would intersect the handle, the handle is not between the
                // camera's near and far planes
                continue;
            }
            // Otherwise we can interpolate the xy of the start and end positions by the
            // lerp factor to get the cursor position in sprite space!
            let cursor_pos_handle = cursor_start_handle
                .lerp(cursor_end_handle, lerp_factor)
                .xy();

            let hit = match (rect, circle) {
                (Some(rect), None) => rect.0.contains(cursor_pos_handle),
                (None, Some(circle)) => cursor_pos_handle.length() < circle.0.radius,
                _ => {
                    error_once!("Entity {entity:?} has both rect and circle components");
                    continue;
                }
            };

            if hit {
                let hit_pos_world = handle_transform.transform_point(cursor_pos_handle.extend(0.0));
                // Transform point from world to camera space to get the Z distance
                let hit_pos_cam = cam_transform
                    .affine()
                    .inverse()
                    .transform_point3(hit_pos_world);
                // HitData requires a depth as calculated from the camera's near clipping plane
                let depth = -cam_ortho.near - hit_pos_cam.z;
                picks.push((
                    *entity,
                    HitData::new(
                        cam_entity,
                        depth,
                        Some(hit_pos_world),
                        Some(*handle_transform.back()),
                    ),
                ));

                if pickable.map_or(true, |p| p.should_block_lower) {
                    break;
                }
            }
        }

        let order = camera.order as f32;
        output.write(PointerHits::new(*pointer, picks, order));
    }
}
