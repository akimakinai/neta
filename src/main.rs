use bevy::{
    asset::UnapprovedPathMode,
    dev_tools::picking_debug::{DebugPickingMode, DebugPickingPlugin},
    prelude::*,
    render::pipelined_rendering::PipelinedRenderingPlugin,
    window::{PresentMode, PrimaryWindow},
    winit::WinitSettings,
};

mod handle;
mod observe_component;

use bevy_vector_shapes::{
    Shape2dPlugin,
    prelude::ShapePainter,
    shapes::{DiscPainter, RectPainter},
};
use handle::{ControlHandlePlugin, CurrentControlHandle, spawn_control_handle};
use observe_component::Observe;

fn main() {
    App::new()
        .insert_resource(WinitSettings::desktop_app())
        .insert_resource(SpritePickingSettings {
            require_markers: false,
            picking_mode: SpritePickingMode::BoundingBox,
        })
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                })
                .disable::<PipelinedRenderingPlugin>(),
        )
        .insert_resource(DebugPickingMode::Normal)
        .add_plugins(DebugPickingPlugin)
        .add_plugins(Shape2dPlugin::default())
        .add_plugins(ControlHandlePlugin)
        .add_systems(Startup, startup)
        .add_systems(Update, (setup_sprite, draw_border, dummy_paint))
        .run();
}

#[derive(Component)]
struct Canvas;

#[derive(Component)]
struct MainCamera;

fn button(world: &World, label: &str) -> impl Bundle {
    let assets = world.resource::<AssetServer>();
    let button_normal = assets.load("images/tile_0015.png");
    let button_pressed = assets.load("images/tile_0016.png");

    (
        Button,
        Node {
            width: Val::Px(150.0),
            height: Val::Px(35.0),
            // border: UiRect::all(Val::Px(5.0)),
            // horizontally center child text
            justify_content: JustifyContent::Center,
            // vertically center child text
            align_items: AlignItems::Center,
            ..default()
        },
        ImageNode {
            image: button_normal.clone(),
            image_mode: NodeImageMode::Sliced(TextureSlicer {
                border: BorderRect::all(8.),
                sides_scale_mode: SliceScaleMode::Tile { stretch_value: 1.0 },
                ..default()
            }),
            ..default()
        },
        children![(
            Text::new(label),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.9, 0.9)),
            TextShadow::default(),
        )],
        Observe::new(
            move |trigger: Trigger<Pointer<Pressed>>, mut image_node: Query<&mut ImageNode>| {
                image_node.get_mut(trigger.target()).unwrap().image = button_pressed.clone();
            },
        ),
        Observe::new({
            let button_normal = button_normal.clone();
            move |trigger: Trigger<Pointer<Released>>, mut image_node: Query<&mut ImageNode>| {
                image_node.get_mut(trigger.target()).unwrap().image = button_normal.clone();
            }
        }),
        Observe::new(
            move |trigger: Trigger<Pointer<DragEnd>>, mut image_node: Query<&mut ImageNode>| {
                image_node.get_mut(trigger.target()).unwrap().image = button_normal.clone();
            },
        ),
        Observe::new(
            |trigger: Trigger<Pointer<Click>>,
             mut commands: Commands,
             canvas_id: Single<Entity, With<Canvas>>,
             assets: Res<AssetServer>| {
                info!(?trigger);
                let canvas_id = canvas_id.into_inner();
                let files = rfd::FileDialog::new().pick_files();
                info!(?files);
                if let Some(files) = files {
                    for file in files {
                        let img: Handle<Image> = assets.load(file);
                        commands.entity(canvas_id).with_child(ImageFrame(img));
                    }
                }
            },
        ),
    )
}

fn startup(world: &mut World) {
    world.spawn((Camera2d, MainCamera));

    let menu_background = world.resource::<AssetServer>().load("images/tile_0028.png");

    world.spawn((
        Name::new("Menu"),
        Node::default(),
        Pickable::IGNORE,
        children![(
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(5.0),
                left: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(5.0), Val::Px(10.0)),
                ..default()
            },
            ImageNode {
                image: menu_background,
                image_mode: NodeImageMode::Sliced(TextureSlicer {
                    border: BorderRect::all(8.),
                    sides_scale_mode: SliceScaleMode::Tile { stretch_value: 1.0 },
                    ..default()
                }),
                ..default()
            },
            children![button(world, "Add")],
        ),],
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
             current: Option<Res<CurrentControlHandle>>| {
                if trigger.event().button == PointerButton::Primary {
                    if let Some(current_handle) = current {
                        commands.entity(current_handle.0).despawn();
                        commands.remove_resource::<CurrentControlHandle>();
                    }
                }
            },
        );
}

fn dummy_paint(mut painter: ShapePainter, mut done: Local<bool>) {
    // Dummy draw to compile shaders in advance.
    // Missing renders are especially visible with `WinitSettings::desktop_app()`.

    if *done {
        return;
    }
    *done = true;

    painter.circle(0.0);
    painter.rect(Vec2::splat(0.0));
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
        transform.scale *= 1.1;
    } else {
        transform.scale /= 1.1;
    }
}

fn drag_with_middle_mouse_button(
    trigger: Trigger<Pointer<Drag>>,
    mut camera: Query<(&mut Transform, &Camera, &GlobalTransform), With<MainCamera>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) {
    if mouse_buttons.any_pressed([MouseButton::Left, MouseButton::Right]) {
        return;
    }

    let event = trigger.event();
    if event.button == PointerButton::Middle {
        let Ok((mut transform, camera, camera_transform)) = camera.single_mut() else {
            return;
        };

        if let Some(world_delta) =
            viewport_delta_to_world_2d(camera, camera_transform, trigger.delta)
        {
            transform.translation -= world_delta.extend(0.0);
        }
    }
}

#[derive(Component)]
struct ImageFrame(Handle<Image>);

#[derive(Component)]
struct Hovered;

fn setup_sprite(
    mut commands: Commands,
    images: Res<Assets<Image>>,
    image_frames: Query<(Entity, &ImageFrame), Without<Sprite>>,
    mut index: Local<u32>,
) {
    for (entity, image_frame) in image_frames {
        let Some(image) = images.get(&image_frame.0) else {
            continue;
        };
        let size = image.texture_descriptor.size;

        let id = commands
            .entity(entity)
            .insert((
                Sprite {
                    image: image_frame.0.clone(),
                    custom_size: Some(Vec2::new(size.width as f32, size.height as f32)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, *index as f32 / 65536.0),
                Pickable::default(),
            ))
            .observe(
                |trigger: Trigger<Pointer<Drag>>,
                 mut transform: Query<&mut Transform>,
                 camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>| {
                    if trigger.event().button != PointerButton::Primary {
                        return;
                    }

                    let Ok(mut sprite_tr) = transform.get_mut(trigger.target()) else {
                        return;
                    };

                    let (camera, camera_transform) = camera.into_inner();

                    if let Some(world_delta) =
                        viewport_delta_to_world_2d(camera, camera_transform, trigger.delta)
                    {
                        sprite_tr.translation += world_delta.extend(0.0);
                    }
                },
            )
            .observe(|trigger: Trigger<Pointer<Over>>, mut commands: Commands| {
                commands.entity(trigger.target()).insert(Hovered);
            })
            .observe(|trigger: Trigger<Pointer<Out>>, mut commands: Commands| {
                commands.entity(trigger.target()).remove::<Hovered>();
            })
            .observe(
                |mut trigger: Trigger<Pointer<Click>>, mut commands: Commands| {
                    trigger.propagate(false);
                    info!(?trigger);
                    commands.queue(spawn_control_handle(trigger.target()));
                },
            )
            .id();
        info!("Spawned sprite with id: {id:?}");

        *index += 1;
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

fn draw_border(
    hovered: Query<(&GlobalTransform, &Sprite), With<Hovered>>,
    mut painter: ShapePainter,
) {
    for (transform, sprite) in hovered {
        let size = sprite.custom_size.unwrap_or(Vec2::new(0.0, 0.0));
        painter.transform = transform.compute_transform();
        painter.hollow = true;
        painter.corner_radii = Vec4::splat(5.0);
        painter.rect(size);
    }
}
