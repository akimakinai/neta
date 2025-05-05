use crate::{
    canvas::{Canvas, ImageFrame},
    observe_component::Observe,
};
use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(world: &mut World) {
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
                padding: UiRect::axes(Val::Px(10.0), Val::Px(10.0)),
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
        button_observers(
            button_normal,
            button_pressed,
            |_trigger: Trigger<Pointer<Click>>,
             mut commands: Commands,
             canvas_id: Query<Entity, With<Canvas>>,
             assets: Res<AssetServer>| {
                let canvas_id = canvas_id.single().unwrap();
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

/// A set of observers for a button.
/// It will change the button image when pressed and released.
/// Also, it will call the `on_click` function when the button is clicked.
fn button_observers<M>(
    button_normal: Handle<Image>,
    button_pressed: Handle<Image>,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) -> impl Bundle {
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
        Observe::new(on_click),
    )
}
