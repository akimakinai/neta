#![allow(unused)]

use bevy::{
    ecs::system::SystemParam, prelude::*, render::camera::NormalizedRenderTarget,
    window::PrimaryWindow,
};

use crate::bevyhow;

use super::{ControlCamera, MainCamera};

/// Helper for translating between two cameras' viewports.
/// Two cameras are expected to target the same window and have the same viewport size.
#[derive(SystemParam)]
pub struct CameraTranslator<'w, 's> {
    transform_helper: TransformHelper<'w, 's>,
    main_camera: Single<'w, (&'static Camera, Entity), With<MainCamera>>,
    control_camera: Single<'w, (&'static Camera, Entity), With<ControlCamera>>,
}

impl<'w, 's> CameraTranslator<'w, 's> {
    /// Maps a [`GlobalTransform`] from the [`MainCamera`]'s view into the [`ControlCamera`]'s view,
    /// based on viewport coordinate assuming two cameras target the same window.
    /// This is useful when you want to visually align objects between two camera spaces.
    ///
    /// The returned `Transform` will always have a `z` value of 0.0.
    pub fn to_control(&self, main_transform: &GlobalTransform) -> Result<Transform> {
        let main_camera_transform = self
            .transform_helper
            .compute_global_transform(self.main_camera.1)?;

        let main_viewport = self
            .main_camera
            .0
            .world_to_viewport(&main_camera_transform, main_transform.translation())?;

        let control_camera_transform = self
            .transform_helper
            .compute_global_transform(self.control_camera.1)?;

        let translation = self
            .control_camera
            .0
            .viewport_to_world_2d(&control_camera_transform, main_viewport)?;

        let affine = main_transform.affine()
            * control_camera_transform.affine()
            * main_camera_transform.affine().inverse();
        let (scale, rotation, _) = affine.to_scale_rotation_translation();

        Ok(Transform {
            translation: translation.extend(0.0),
            scale,
            rotation,
        })
    }

    pub fn to_main(&self, control_transform: &GlobalTransform) -> Result<Transform> {
        let control_camera_transform = self
            .transform_helper
            .compute_global_transform(self.control_camera.1)?;

        let control_viewport = self
            .control_camera
            .0
            .world_to_viewport(&control_camera_transform, control_transform.translation())?;

        let main_camera_transform = self
            .transform_helper
            .compute_global_transform(self.main_camera.1)?;

        let translation = self
            .main_camera
            .0
            .viewport_to_world_2d(&main_camera_transform, control_viewport)?;

        let affine = control_transform.affine()
            * main_camera_transform.affine()
            * control_camera_transform.affine().inverse();
        let (scale, rotation, _) = affine.to_scale_rotation_translation();

        Ok(Transform {
            translation: translation.extend(0.0),
            scale,
            rotation,
        })
    }

    pub fn map_rect_to_main(&self, rect: &Rect) -> Result<Rect> {
        let control_camera_transform = self
            .transform_helper
            .compute_global_transform(self.control_camera.1)?;

        let main_camera_transform = self
            .transform_helper
            .compute_global_transform(self.main_camera.1)?;

        let affine = main_camera_transform.affine() * control_camera_transform.affine().inverse();

        Ok(Rect {
            min: affine.transform_point3(rect.min.extend(0.0)).truncate(),
            max: affine.transform_point3(rect.max.extend(0.0)).truncate(),
        })
    }
}

// Since the information on which camera the picking backend used is not included in pointer events,
// we need to specify marker component to find the intended camera.
#[derive(SystemParam)]
pub struct RenderTargetHelper<'w, 's, C: Component> {
    camera: Query<'w, 's, (Entity, &'static Camera), With<C>>,
    primary_window: Query<'w, 's, Entity, With<PrimaryWindow>>,
}

impl<'w, 's, C: Component> RenderTargetHelper<'w, 's, C> {
    pub fn find_camera(&self, target: &NormalizedRenderTarget) -> Result<Entity> {
        let primary_window = self.primary_window.single().ok();

        let (id, _) = self
            .camera
            .iter()
            .find(|(_id, camera)| camera.target.normalize(primary_window).as_ref() == Some(target))
            .ok_or_else(|| bevyhow!("Camera not found for target {target:?}"))?;

        Ok(id)
    }
}
