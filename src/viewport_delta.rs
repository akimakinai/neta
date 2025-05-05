use bevy::{
    ecs::{query::QueryFilter, system::SystemParam},
    picking::pointer::Location,
    prelude::*,
    window::PrimaryWindow,
};

#[derive(SystemParam)]
pub struct PointerDelta<'w, 's, F: QueryFilter + 'static = ()> {
    camera: Query<'w, 's, (Entity, &'static Camera, &'static GlobalTransform), F>,
    primary_window: Query<'w, 's, Entity, With<PrimaryWindow>>,
}

impl<'w, 's, F: QueryFilter> PointerDelta<'w, 's, F> {
    /// Returns the world delta converted from the viewport delta and the camera entity of the pointer.
    pub fn get_world(&self, pointer_location: &Location, delta: Vec2) -> Option<(Vec2, Entity)> {
        let (camera_id, camera, camera_transform) = self
            .camera
            .iter()
            .find(|(_, camera, _)| pointer_location.is_in_viewport(camera, &self.primary_window))?;
        Some((
            viewport_delta_to_world_2d(camera, camera_transform, delta)?,
            camera_id,
        ))
    }
}

fn viewport_delta_to_world_2d(
    camera: &Camera,
    cam_t: &GlobalTransform,
    viewport_delta: Vec2,
) -> Option<Vec2> {
    let rect = camera.logical_viewport_rect()?;
    let mut ndc_delta = viewport_delta / rect.size();
    ndc_delta.y = -ndc_delta.y;
    ndc_delta *= 2.0;

    let ndc_to_world = cam_t.compute_matrix() * camera.clip_from_view().inverse();

    let right = ndc_to_world.x_axis.truncate();
    let up = ndc_to_world.y_axis.truncate();

    Some((right * ndc_delta.x + up * ndc_delta.y).truncate())
}
