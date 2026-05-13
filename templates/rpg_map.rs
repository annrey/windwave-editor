use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_map);
    }
}

fn spawn_map(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    // Create a simple tilemap
    let texture_handle = asset_server.load("tiles.png");
    let atlas = TextureAtlas::from_grid(
        texture_handle,
        Vec2::new(32.0, 32.0),
        4,
        1,
        None,
        None,
    );
    let atlas_handle = texture_atlases.add(atlas);
    
    let map_size = TilemapSize { x: 20, y: 15 };
    let tile_size = TilemapTileSize { x: 32.0, y: 32.0 };
    
    let mut tile_storage = TileStorage::empty(map_size);
    let tilemap_entity = commands.spawn_empty().id();
    
    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    tilemap_id: TilemapId(tilemap_entity),
                    ..default()
                })
                .id();
            tile_storage.set(&tile_pos, tile_entity);
        }
    }
    
    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size: tile_size.into(),
        size: map_size,
        storage: tile_storage,
        texture: TilemapTexture::Single(asset_server.load("tiles.png")),
        tile_size,
        transform: Transform::from_xyz(-320.0, -240.0, 0.0),
        ..default()
    });
}
