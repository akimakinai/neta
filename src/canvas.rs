use crate::handle::ControlHandlePlugin;
use crate::observe_component::Observe;
use crate::viewport_delta::PointerDelta;
use bevy::{ecs::schedule::common_conditions, prelude::*, window::PrimaryWindow};
use bevy_vector_shapes::{
    Shape2dPlugin,
    prelude::ShapePainter,
    shapes::{DiscPainter, RectPainter},
};

use crate::handle;

pub struct CanvasPlugin;

impl Plugin for CanvasPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpritePickingSettings {
            require_markers: false,
            picking_mode: SpritePickingMode::BoundingBox,
        })
        .add_plugins(Shape2dPlugin::default())
        .add_plugins(ControlHandlePlugin)
        .add_systems(Startup, startup)
        .add_systems(
            Update,
            (
                setup_sprite,
                dummy_paint.run_if(common_conditions::run_once),
            ),
        )
        .add_systems(
            PostUpdate,
            draw_border
                .after(TransformSystem::TransformPropagate)
                .run_if(
                    |current: Option<Res<crate::handle::CurrentControlHandle>>| current.is_none(),
                ),
        );
    }
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
        .observe(|trigger: Trigger<Pointer<Click>>, mut commands: Commands| {
            if trigger.event().button == PointerButton::Primary {
                commands.queue(handle::despawn_control_handle);
            }
        });
}

fn dummy_paint(mut painter: ShapePainter) {
    // Dummy draw to compile shaders in advance.
    // Missing renders are especially visible with `WinitSettings::desktop_app()`.

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
    mut camera: Query<&mut Transform, With<Camera>>,
    pointer_delta: PointerDelta,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) {
    if mouse_buttons.any_pressed([MouseButton::Left, MouseButton::Right]) {
        return;
    }

    let event = trigger.event();
    if event.button == PointerButton::Middle {
        if let Some((world_delta, camera_id)) =
            pointer_delta.get_world(&trigger.pointer_location, trigger.delta)
        {
            let Ok(mut transform) = camera.get_mut(camera_id) else {
                return;
            };

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
                 viewport_delta: PointerDelta| {
                    if trigger.event().button != PointerButton::Primary {
                        return;
                    }

                    let Ok(mut sprite_tr) = transform.get_mut(trigger.target()) else {
                        return;
                    };

                    if let Some((world_delta, _)) =
                        viewport_delta.get_world(&trigger.pointer_location, trigger.delta)
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
                    commands.queue(handle::spawn_control_handle(trigger.target()));
                },
            )
            .id();
        info!("Spawned sprite with id: {id:?}");

        *index += 1;
    }
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
