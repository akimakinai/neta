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
    pub fn to_control_world_pos(&self, main_world_pos: Vec3) -> Result<Vec3> {
        let main_camera = self.main_camera.single()?;
        let control_camera = self.control_camera.single()?;

        let Ok(main_viewport) = main_camera
            .0
            .world_to_viewport(main_camera.1, main_world_pos)
        else {
            return Err("`world_to_viewport` failed".into());
        };

        let Ok(control_world) = control_camera
            .0
            .viewport_to_world_2d(control_camera.1, main_viewport)
        else {
            return Err("`viewport_to_world_2d` failed".into());
        };

        let control_world = control_world.extend(0.0);
        Ok(control_world)
    }

    pub fn to_control_scale(&self) -> Result<Vec3> {
        let main_camera = self.main_camera.single()?;
        let control_camera = self.control_camera.single()?;

        Ok(control_camera.1.scale() / main_camera.1.scale())
    }

    // TODO: Transform -> Transform
}
