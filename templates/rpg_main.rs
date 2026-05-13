use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

mod map;
use map::MapPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TilemapPlugin)
        .add_plugins(MapPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, player_movement)
        .run();
}

#[derive(Component)]
struct Player {
    speed: f32,
}

fn setup(mut commands: Commands) {
    // Camera
    commands.spawn(Camera2dBundle::default());
    
    // Player
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(0.2, 0.4, 0.8),
                custom_size: Some(Vec2::new(32.0, 32.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, 1.0),
            ..default()
        },
        Player { speed: 150.0 },
    ));
    
    // Lighting
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 2000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 100.0, 100.0),
        ..default()
    });
}

fn player_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&Player, &mut Transform)>,
    time: Res<Time>,
) {
    for (player, mut transform) in query.iter_mut() {
        let mut direction = Vec3::ZERO;
        
        if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
            direction.y += 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
            direction.y -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
            direction.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
            direction.x += 1.0;
        }
        
        if direction != Vec3::ZERO {
            direction = direction.normalize();
            transform.translation += direction * player.speed * time.delta_seconds();
        }
    }
}
