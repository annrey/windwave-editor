use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(Update, player_movement);
    }
}

#[derive(Component)]
pub struct Player {
    speed: f32,
    jump_force: f32,
}

fn spawn_player(mut commands: Commands) {
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(0.8, 0.2, 0.2),
                custom_size: Some(Vec2::new(50.0, 50.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        Player {
            speed: 200.0,
            jump_force: 400.0,
        },
        RigidBody::Dynamic,
        Collider::cuboid(25.0, 25.0),
        Velocity::zero(),
    ));
}

fn player_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&Player, &mut Velocity)>,
) {
    for (player, mut velocity) in query.iter_mut() {
        let mut direction = 0.0;
        
        if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
            direction -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
            direction += 1.0;
        }
        
        velocity.linvel.x = direction * player.speed;
        
        if keyboard_input.just_pressed(KeyCode::Space) || keyboard_input.just_pressed(KeyCode::ArrowUp) {
            velocity.linvel.y = player.jump_force;
        }
    }
}
