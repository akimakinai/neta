use bevy::{picking::backend::prelude::*, prelude::*, render::view::RenderLayers};

/// Picking plugin for invisible hover area.
#[derive(Default, Clone, Resource)]
pub struct AreaPickingPlugin {
    pub require_markers: bool,
}

impl Plugin for AreaPickingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.clone())
            .add_systems(PreUpdate, pick_shape.in_set(PickSet::Backend));
    }
}

/// Defines a circular picking area.
#[derive(Component)]
#[require(Transform)]
pub struct PickingAreaCircle(pub Circle);

fn pick_shape(
    ray_map: Res<RayMap>,
    cameras: Query<(
        Entity,
        &Camera,
        &GlobalTransform,
        &Projection,
        Option<&RenderLayers>,
    )>,
    handle_shapes: Query<(
        Entity,
        &GlobalTransform,
        &PickingAreaCircle,
        Option<&Pickable>,
        Option<&RenderLayers>,
    )>,
    mut output: EventWriter<PointerHits>,
    settings: Res<AreaPickingPlugin>,
) {
    // based on bevy_sprite\src\picking_backend.rs

    let mut sorted_handles = handle_shapes
        .iter()
        .filter(|(_, transform, .., pickable, _)| {
            !transform.affine().is_nan()
                && (!settings.require_markers || pickable.is_some_and(|p| p.is_hoverable))
        })
        .collect::<Vec<_>>();
    radsort::sort_by_key(&mut sorted_handles, |(_, transform, ..)| {
        -transform.translation().z
    });

    for (ray_id, ray) in ray_map.iter() {
        let Ok((
            cam_entity,
            camera,
            cam_transform,
            Projection::Orthographic(cam_ortho),
            camera_render_layers,
        )) = cameras.get(ray_id.camera)
        else {
            continue;
        };

        let camera_render_layers = camera_render_layers.unwrap_or_default();

        let mut picks = vec![];

        for (entity, handle_transform, circle, pickable, render_layers) in &sorted_handles {
            if !render_layers
                .unwrap_or_default()
                .intersects(camera_render_layers)
            {
                continue;
            }

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

                // Entities without the `Pickable` component block by default.
                if pickable.is_none_or(|p| p.should_block_lower) {
                    break;
                }
            }
        }

        let order = camera.order as f32;
        output.write(PointerHits::new(ray_id.pointer, picks, order));
    }
}
