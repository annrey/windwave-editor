use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

mod player;
use player::PlayerPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_plugins(PlayerPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // Camera
    commands.spawn(Camera2dBundle::default());
    
    // Ground
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(0.3, 0.5, 0.3),
                custom_size: Some(Vec2::new(800.0, 50.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, -200.0, 0.0),
            ..default()
        },
        RigidBody::Fixed,
        Collider::cuboid(400.0, 25.0),
    ));
    
    // Lighting
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 200.0, 100.0),
        ..default()
    });
}
