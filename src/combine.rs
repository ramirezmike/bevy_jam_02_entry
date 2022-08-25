use bevy::prelude::*;
use crate::{
    AppState, maze::CornStalk, assets::GameAssets, component_adder::AnimationLink,
    collision, game_state, ZeroSignum,
};
use bevy::render::primitives::Aabb;
use rand::thread_rng;
use rand::prelude::SliceRandom;
use std::f32::consts::{TAU, PI};

pub struct CombinePlugin;
impl Plugin for CombinePlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_update(AppState::InGame)
                .with_system(animate_combine)
                .with_system(harvest_corn)
                .with_system(handle_corn_collision)
        );
    }
}

#[derive(PartialEq)]
pub enum Heading {
    Left,
    Right,
    Up,
    Down,
}

impl Default for Heading {
    fn default() -> Self {
        Heading::Right
    }
}

#[derive(Component)]
pub struct Combine {
    pub animation_set: bool,
    pub heading: Heading,
    pub velocity: Vec3,
    pub speed: f32,
    pub current_rotation_time: f32,
    pub target_rotation: Quat,
    pub target_x_coordinate: f32,
    pub friction: f32,
}

impl Default for Combine {
    fn default() -> Self {
        Combine {
            animation_set: false,
            velocity: Vec3::default(),
            speed: 20.0,
            current_rotation_time: 0.0,
            heading: Heading::Right,
            target_rotation: Quat::from_rotation_y(TAU * 0.75),
            target_x_coordinate: 0.0,
            friction: 0.01,
        }
    }
}

#[derive(Component)]
pub struct CombineBlade;

const CORN_CUT_DISTANCE: f32 = 0.7;
fn handle_corn_collision( 
    mut commands: Commands,
    mut corns: Query<(Entity, &mut CornStalk, &mut Transform), Without<Combine>>,
    combine_blades: Query<(&Transform, &CombineBlade, &Aabb, &GlobalTransform), Without<CornStalk>>,
) {
    for (blade_transform, blade, blade_aabb, blade_global_transform) in &combine_blades {
        let blade_global_matrix = blade_global_transform.compute_matrix();
        let blade_inverse_transform_matrix = blade_global_matrix.inverse();
        let min: Vec3 = blade_aabb.min().into();
        let max: Vec3 = blade_aabb.max().into();

        for (entity, mut corn, mut corn_transform) in &mut corns {
            if corn.is_harvested { continue; }

            let corn_translation = corn_transform.translation;
            let corn_inverse = blade_inverse_transform_matrix.transform_point3(corn_translation);

            let corn_in_hitbox = corn_inverse.x > min.x
                              && corn_inverse.x < max.x
                              && corn_inverse.z > min.z
                              && corn_inverse.z < max.z;

            if corn_in_hitbox {
                corn_transform.scale.y = 0.1;
//              corn.is_harvested = true;
//              commands.entity(entity)
//                      .remove::<collision::Collidable>();
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}


fn harvest_corn(
    mut combines: Query<(&mut Combine, &mut Transform), Without<CornStalk>>,
    corns: Query<(&CornStalk, &Transform)>,
    game_state: Res<game_state::GameState>,
    time: Res<Time>,
) {
    for (mut combine, mut combine_transform) in &mut combines {
        match combine.heading {
            Heading::Left | Heading::Right => {
                if (combine_transform.translation.z < -(game_state.maze_size / 2.0) && combine.heading == Heading::Left)
                || (combine_transform.translation.z > game_state.maze_size / 2.0 && combine.heading == Heading::Right) {
                    combine.current_rotation_time = 0.0;
                    let unharvested_corn = corns.iter()
                                                .filter(|(c, _)| !c.is_harvested)
                                                .collect::<Vec::<_>>();
                    let mut rng = thread_rng();

                    if unharvested_corn.is_empty() {
                        println!("no more corn :(");
                        // end of round?
                    }

                    let corn_transform = unharvested_corn.choose(&mut rng).map(|(_, t)| *t);
                    combine.target_x_coordinate =
                        if let Some(corn_transform) = corn_transform  {
                            corn_transform.translation.x
                        } else {
                            println!("corn issue??");
                            0.0
                        };

                    combine.velocity.z = 0.0;
                    if combine.target_x_coordinate > combine_transform.translation.x {
                        combine.target_rotation = Quat::from_rotation_y(0.0);
                        combine.heading = Heading::Up;
                    } else {
                        combine.target_rotation = Quat::from_rotation_y(TAU * 0.5); 
                        combine.heading = Heading::Down;
                    }
                }
            },
            Heading::Up | Heading::Down => {
                if (combine_transform.translation.x - combine.target_x_coordinate).abs() < 0.1 {
                    combine.heading = if combine_transform.translation.z > 0.0 { Heading::Left } else { Heading::Right };
                    combine.current_rotation_time = 0.0;
                    combine.velocity.x = 0.0;
                    combine.target_rotation = match combine.heading {
                        Heading::Left => Quat::from_rotation_y(TAU * 0.25),
                        Heading::Right => Quat::from_rotation_y(TAU * 0.75),
                        _ => combine.target_rotation
                    };
                }
            },
        }

        combine.current_rotation_time += time.delta_seconds();
        combine.current_rotation_time = combine.current_rotation_time.clamp(0.0, 3.0);
        if combine.current_rotation_time <= 1.1 {
            let rotation = combine_transform.rotation.lerp(combine.target_rotation, combine.current_rotation_time);

            if !rotation.is_nan() {
                combine_transform.rotation = rotation;
            }
        } else {
            let speed: f32 = combine.speed;
            let friction: f32 = combine.friction;

            combine.velocity *= friction.powf(time.delta_seconds());

            let direction = combine_transform.right();
            let acceleration = Vec3::from(direction);
            combine.velocity += (acceleration.zero_signum() * speed) * time.delta_seconds();

            let new_translation = combine_transform.translation + (combine.velocity * time.delta_seconds());
            combine_transform.translation = new_translation;
        }
    }
}

fn animate_combine(
    mut combines: Query<(&mut Combine, &AnimationLink)>,
    mut animations: Query<&mut AnimationPlayer>,
    game_assets: ResMut<GameAssets>,
) {
    for (mut combine, animation_link) in &mut combines {
        if let Some(animation_entity) = animation_link.entity {
            if let Ok(mut animation) = animations.get_mut(animation_entity) {
                if !combine.animation_set {
                    animation.play(game_assets.combine_drive.clone_weak()).repeat();
                    combine.animation_set = true;
                }

                if animation.is_paused() {
                    animation.resume();
                }
            }
        }
    }
}
