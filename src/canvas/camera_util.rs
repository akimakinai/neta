use bevy::{ecs::system::SystemParam, prelude::*};

use super::{ControlCamera, MainCamera};

#[derive(SystemParam)]
pub struct CameraTranslator<'w, 's> {
    main_camera: Query<
        'w,
        's,
        (&'static Camera, &'static GlobalTransform),
        (With<MainCamera>, Without<ControlCamera>),
    >,
    control_camera: Query<
        'w,
        's,
        (&'static Camera, &'static GlobalTransform),
        (With<ControlCamera>, Without<MainCamera>),
    >,
}

impl<'w, 's> CameraTranslator<'w, 's> {
    /// Maps a [`GlobalTransform`] from the [`MainCamera`]'s view into the [`ControlCamera`]'s view,
    /// based on viewport coordinate assuming two cameras target the same window.
    /// This is useful when you want to visually align objects between two camera spaces.
    ///
    /// The returned `Transform` will always have a `z` value of 0.0.
    pub fn to_control(&self, main_transform: &GlobalTransform) -> Result<Transform> {
        let main_camera = self.main_camera.single()?;
        let control_camera = self.control_camera.single()?;

        let Ok(main_viewport) = main_camera
            .0
            .world_to_viewport(main_camera.1, main_transform.translation())
        else {
            return Err("`world_to_viewport` failed".into());
        };

        let Ok(translation) = control_camera
            .0
            .viewport_to_world_2d(control_camera.1, main_viewport)
        else {
            return Err("`viewport_to_world_2d` failed".into());
        };

        let affine =
            main_transform.affine() * control_camera.1.affine() * main_camera.1.affine().inverse();
        let (scale, rotation, _) = affine.to_scale_rotation_translation();

        Ok(Transform {
            translation: translation.extend(0.0),
            scale,
            rotation,
            // rotation: main_transform.rotation()
            //     * control_camera.1.rotation()
            //     * main_camera.1.rotation().inverse(),
            // scale: main_transform.scale() * control_camera.1.scale() / main_camera.1.scale(),
        })
    }
}
