use bevy::{
    ecs::system::RunSystemOnce,
    picking::{
        PickSet,
        backend::{HitData, PointerHits, ray::RayMap},
    },
    prelude::*,
    sprite::Anchor,
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
            .add_systems(Update, update_corner_handle);
    }
}

/// A circle area data used for picking the control handle.
#[derive(Component)]
#[require(Transform)]
pub struct PickingAreaCircle(pub Circle);

#[derive(Component)]
pub struct ControlHandle;

#[derive(Resource)]
pub struct CurrentControlHandle(pub Entity);

#[derive(Component)]
struct ControlHandleCorner(Anchor);

const CORNER_HANDLE_RADIUS: f32 = 5.0;

pub fn spawn_control_handle(parent: Entity) -> impl Command<Result> {
    move |world: &mut World| -> Result {
        // TODO: cache system
        world.run_system_once(
            move |mut commands: Commands, current: Option<Res<CurrentControlHandle>>| {
                if let Some(current_handle) = current {
                    commands.entity(current_handle.0).despawn();
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
                        // Anchor::TopCenter,
                        // Anchor::CenterLeft,
                        // Anchor::CenterRight,
                        // Anchor::BottomCenter,
                    ] {
                        parent
                            .spawn((
                                PickingAreaCircle(Circle::new(CORNER_HANDLE_RADIUS)),
                                ControlHandleCorner(anchor),
                                Transform::from_translation(Vec3::new(0., 0., 2.)),
                            ))
                            .observe(|mut trigger: Trigger<Pointer<Drag>>| {
                                trigger.propagate(false);
                            });
                    }
                });

                let handle = handle.id();
                commands.insert_resource(CurrentControlHandle(handle));
            },
        )?;
        Ok(())
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
    child_of: Query<&ChildOf>,
    mut handle: Query<(Entity, &mut Transform, &ControlHandleCorner)>,
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

const HANDLE_WIDTH: f32 = 2.0;

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

        // border
        painter.hollow = true;
        painter.color = Color::WHITE;
        painter.thickness = HANDLE_WIDTH;
        painter.rect(frame_size + Vec2::new(HANDLE_WIDTH, HANDLE_WIDTH));

        painter.hollow = false;
        painter.thickness = 2.0;

        // resizing handles
        for point in [
            Vec2::new(-0.5, -0.5),
            Vec2::new(0.5, -0.5),
            Vec2::new(-0.5, 0.5),
            Vec2::new(0.5, 0.5),
        ] {
            painter.transform.translation = frame_transform
                .transform_point((point * sprite_size).extend(0.))
                .with_z(3.0);
            painter.hollow = false;
            painter.thickness = 0.0;
            painter.color = Color::WHITE;
            painter.circle(CORNER_HANDLE_RADIUS);

            painter.hollow = true;
            painter.color = Color::BLACK;
            painter.thickness = 1.0;
            painter.circle(CORNER_HANDLE_RADIUS + painter.thickness / 2.);
        }
    }
}

fn pick_handle(
    ray_map: Res<RayMap>,
    cameras: Query<(Entity, &Camera, &GlobalTransform, &Projection)>,
    handle_shapes: Query<(
        Entity,
        &GlobalTransform,
        &PickingAreaCircle,
        Option<&Pickable>,
    )>,
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

    for (ray_id, ray) in ray_map.iter() {
        let Ok((cam_entity, camera, cam_transform, Projection::Orthographic(cam_ortho))) =
            cameras.get(ray_id.camera)
        else {
            continue;
        };

        let mut picks = vec![];

        for (entity, handle_transform, circle, pickable) in &sorted_handles {
            // Transform cursor line segment to handle coordinate system
            let world_to_handle = handle_transform.affine().inverse();

            let Some(cursor_pos_handle) = ray
                .intersect_plane(
                    handle_transform.translation(),
                    InfinitePlane3d::new(handle_transform.back()),
                )
                .map(|distance| {
                    world_to_handle
                        .transform_point3(ray.get_point(distance))
                        .xy()
                })
            else {
                continue;
            };

            let hit = cursor_pos_handle.length() < circle.0.radius;

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

                if pickable.is_none_or(|p| p.should_block_lower) {
                    break;
                }
            }
        }

        let order = camera.order as f32;
        output.write(PointerHits::new(ray_id.pointer, picks, order));
    }
}

// fn handle_gizmo(
//     tr_helper: TransformHelper,
//     handle_shapes: Query<(Entity, &ControlHandleCircle)>,
//     mut gizmos: Gizmos,
// ) {
//     for (entity, circle) in handle_shapes.iter() {
//         let Ok(transform) = tr_helper.compute_global_transform(entity) else {
//             continue;
//         };

//         let color = Color::linear_rgb(1.0, 0.0, 0.0);
//         // gizmos.rect(transform.to_isometry(), rect.0.size(), color);
//         gizmos.circle(transform.to_isometry(), circle.0.radius, color);
//     }
// }
