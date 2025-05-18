#![allow(unused)]

use std::{sync::Mutex, time::Duration};

use bevy::prelude::*;

static GIZMO_COMMANDS: Mutex<Vec<(Timer, Box<dyn FnMut(&mut Gizmos) + Send>)>> =
    Mutex::new(Vec::new());

const GIZMO_TIMEOUT: Duration = Duration::from_secs(30);

pub struct DebugGizmoPlugin;

impl Plugin for DebugGizmoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, execute_gizmo_commands);
    }
}

fn execute_gizmo_commands(mut gizmos: Gizmos, time: Res<Time>) {
    let mut commands = GIZMO_COMMANDS.lock().unwrap();
    commands.retain_mut(|command| {
        if command.0.tick(time.delta()).just_finished() {
            return false;
        }
        command.1(&mut gizmos);
        true
    });
}

pub fn debug_gizmo<F>(command: F)
where
    F: FnMut(&mut Gizmos) + Send + 'static,
{
    let mut commands = GIZMO_COMMANDS.lock().unwrap();
    commands.push((
        Timer::new(GIZMO_TIMEOUT, TimerMode::Once),
        Box::new(command),
    ));
}
