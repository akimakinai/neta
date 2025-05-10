use crate::{
    canvas::{Canvas, Hovered, ImageFrame},
    observe_component::Observe,
};
use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, despawn_dummy.run_if(run_once_at(1)))
            .add_observer(on_click);
    }
}

#[derive(Component)]
struct ContextMenu;

/// Only show this item for the context menu on a frame
#[derive(Component)]
struct FrameContextItem;

/// Only show this item for the context menu on the canvas
#[derive(Component)]
struct CanvasContextItem;

fn on_add_button_clicked(
    _trigger: Trigger<Pointer<Click>>,
    mut commands: Commands,
    canvas_id: Query<Entity, With<Canvas>>,
    assets: Res<AssetServer>,
) {
    let canvas_id = canvas_id.single().unwrap();
    let files = rfd::FileDialog::new().pick_files();
    info!(?files);
    if let Some(files) = files {
        for file in files {
            let img: Handle<Image> = assets.load(file);
            commands.entity(canvas_id).with_child(ImageFrame(img));
        }
    }
}

#[derive(Component)]
struct DummyForShaderInit;

/// Run condition to run the system only once at the `n`-th frame.
/// `run_once_nth(0)` is equivalent to `run_once()`.
fn run_once_at(n: u32) -> impl Condition<()> {
    IntoSystem::into_system(move |mut count: Local<u32>| {
        if *count > n {
            return false;
        }
        let prev_count = *count;
        *count += 1;
        prev_count == n
    })
}

fn despawn_dummy(mut commands: Commands, dummy: Query<Entity, With<DummyForShaderInit>>) {
    for entity in dummy.iter() {
        commands.entity(entity).despawn();
    }
}

fn setup(world: &mut World) {
    let menu_background = world.resource::<AssetServer>().load("images/tile_0028.png");

    let menu_background_node = ImageNode {
        image: menu_background,
        image_mode: NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(8.),
            sides_scale_mode: SliceScaleMode::Tile { stretch_value: 1.0 },
            ..default()
        }),
        ..default()
    };

    // spawn a dummy entity to fix 1-frame delay
    world.spawn((DummyForShaderInit, menu_background_node.clone()));

    world.spawn((
        Name::new("ContextMenu"),
        ContextMenu,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(5.0),
            left: Val::Px(5.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        menu_background_node.clone(),
        children![
            (
                CanvasContextItem,
                button(world, "Add"),
                Observe::new(on_add_button_clicked)
            ),
            (FrameContextItem, button(world, "Remove")),
            button(world, "Organize")
        ],
    ));
}

fn update_context_item_display(
    mut set: ParamSet<(
        Query<&mut Node, With<CanvasContextItem>>,
        Query<&mut Node, With<FrameContextItem>>,
    )>,
    hovered: Query<(), With<Hovered>>,
) {
    let on_canvas = hovered.is_empty();

    let (canvas_display, frame_display) = if on_canvas {
        (Display::default(), Display::None)
    } else {
        (Display::None, Display::default())
    };

    for mut node in set.p0().iter_mut() {
        node.display = canvas_display;
    }

    for mut node in set.p1().iter_mut() {
        node.display = frame_display;
    }
}

/// Create a button with the given label.
fn button(world: &World, label: &str) -> impl Bundle {
    let assets = world.resource::<AssetServer>();
    let button_normal = assets.load("images/tile_0015.png");
    let button_pressed = assets.load("images/tile_0016.png");

    (
        Button,
        Node {
            width: Val::Px(150.0),
            height: Val::Px(35.0),
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
        button_observers(button_normal, button_pressed),
    )
}

/// A set of observers for a button.
/// The button image will be changed when pressed or released.
fn button_observers(button_normal: Handle<Image>, button_pressed: Handle<Image>) -> impl Bundle {
    (
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
    )
}

fn on_click(
    trigger: Trigger<Pointer<Click>>,
    mut commands: Commands,
    mut context_menu: Query<(&mut Node, &mut Visibility), With<ContextMenu>>,
) {
    let Ok((mut node, mut visibility)) = context_menu.single_mut() else {
        return;
    };

    if trigger.button != PointerButton::Secondary {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }

    // Secondary button clicked

    commands.run_system_cached(update_context_item_display);

    let position = trigger.pointer_location.position;
    node.left = Val::Px(position.x);
    node.top = Val::Px(position.y);

    visibility.set_if_neq(Visibility::Inherited);
}
