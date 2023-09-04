use crate::utils::get_creep_type;
use log::*;
use screeps::{game, Part};
use std::collections::HashMap;

pub struct SpawnGoal {
    pub name: String,
    pub body: Vec<Part>,
    pub additive_body: Vec<Part>,
    pub count: u32,
}

pub type SpawnGoals = Vec<SpawnGoal>;

pub struct SpawnManager {
    pub spawn_goals: SpawnGoals,
}

impl SpawnManager {
    pub fn new(spawn_goals: SpawnGoals) -> SpawnManager {
        SpawnManager { spawn_goals }
    }

    pub fn spawn_creeps(&self) {
        let mut additional = 0;

        let creeps = game::creeps();
        let creeps_per_room_by_type = creeps.values().fold(HashMap::new(), |mut acc, creep| {
            let creep_type = get_creep_type(&creep);
            let room_name = creep.room().unwrap().name();
            let count: &mut HashMap<String, u32> = acc.entry(room_name).or_default();
            let creep_count = count.entry(creep_type).or_insert(0);
            *creep_count += 1;
            acc
        });

        for spawn in game::spawns().values() {
            let room_name = spawn.room().unwrap().name();
            let creep_counts = creeps_per_room_by_type.get(&room_name);

            for spawn_goal in self.spawn_goals.iter() {
                let mut count = 0;
                if let Some(creep_counts) = creep_counts {
                    if let Some(creep_count) = creep_counts.get(&spawn_goal.name) {
                        count = *creep_count;
                    }
                }
                let source_count: u32 = spawn
                    .room()
                    .unwrap()
                    .find(screeps::constants::find::SOURCES, None)
                    .len() as u32;
                let target_count = spawn_goal.count * source_count;
                if count < target_count {
                    info!("Spawning {} [{}/{}]", spawn_goal.name, count, target_count);

                    let creep_name = format!("{}-{}-{}", spawn_goal.name, game::time(), additional);
                    let room = spawn.room().unwrap();
                    let body_cost = spawn_goal.body.iter().map(|p| p.cost()).sum::<u32>();
                    let additive_parts_cost = spawn_goal
                        .additive_body
                        .iter()
                        .map(|p| p.cost())
                        .sum::<u32>()
                        + 1;
                    let mut body_parts = spawn_goal.body.clone();

                    if room.energy_available() >= body_cost {
                        if !spawn_goal.additive_body.is_empty() {
                            let remaining_energy =
                                std::cmp::max(room.energy_available() - body_cost, 0);
                            let times_to_add = remaining_energy / additive_parts_cost;
                            info!(
                                "Upgrading the {} creep {} times for an additional {} energy",
                                spawn_goal.name,
                                times_to_add,
                                times_to_add * (additive_parts_cost - 1)
                            );
                            for _ in 0..times_to_add {
                                for part in spawn_goal.additive_body.iter() {
                                    body_parts.push(*part);
                                }
                            }
                        }
                        match spawn.spawn_creep(&body_parts, &creep_name) {
                            Ok(()) => additional += 1,
                            Err(e) => warn!("couldn't spawn {}: {:?}", spawn_goal.name, e),
                        }
                    }
                }
            }
        }
    }
}
