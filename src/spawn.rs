use crate::utils::get_creep_type;
use log::*;
use screeps::{game, Part, ResourceType, RoomName};
use std::collections::HashMap;

#[derive(Debug)]
pub struct SpawnGoal {
    pub name: String,
    pub body: Vec<Part>,
    pub body_upgrades: Vec<Part>,
    pub max_body_upgrades: u32,
    pub source_modifier: u32, // how the # of additonal sources in the room affect the target count. it's a multiplier
    pub count: u32,
    pub is_global: bool,
}

pub type SpawnGoals = Vec<SpawnGoal>;
pub type RoomSpawnGoals = HashMap<RoomName, SpawnGoals>;
pub type RoomCreepCounts = HashMap<RoomName, HashMap<String, u32>>;

pub struct SpawnManager {
    pub room_spawn_goals: RoomSpawnGoals,
    pub room_creep_counts: RoomCreepCounts,
}

impl SpawnManager {
    pub fn new(room_spawn_goals: RoomSpawnGoals) -> SpawnManager {
        let creeps = game::creeps();
        let room_creep_counts = creeps.values().fold(HashMap::new(), |mut acc, creep| {
            let creep_type = get_creep_type(&creep);
            let room_name = creep.room().unwrap().name();
            let count: &mut HashMap<String, u32> = acc.entry(room_name).or_default();
            let creep_count = count.entry(creep_type).or_insert(0);
            *creep_count += 1;
            acc
        });

        SpawnManager {
            room_spawn_goals,
            room_creep_counts,
        }
    }

    pub fn spawn_creeps(&mut self) {
        let mut additional = 0;

        for spawn in game::spawns().values() {
            if spawn.spawning().is_some() {
                continue;
            }

            let room_name = spawn.room().unwrap().name();

            if (spawn.store().get_free_capacity(Some(ResourceType::Energy)) > 0)
                && self.get_creep_count_in_room(&room_name, "worker") >= 3
            {
                continue;
            }

            let source_count: u32 = spawn
                .room()
                .unwrap()
                .find(screeps::constants::find::SOURCES, None)
                .len() as u32;

            if let Some(spawn_goals) = self.room_spawn_goals.get(&room_name) {
                for spawn_goal in spawn_goals.iter() {
                    let creep_count = if spawn_goal.is_global {
                        self.get_global_creep_count(&spawn_goal.name)
                    } else {
                        self.get_creep_count_in_room(&room_name, &spawn_goal.name)
                    };

                    let target_count = spawn_goal.count
                        + std::cmp::max(
                            spawn_goal.count * (source_count - 1) * spawn_goal.source_modifier,
                            0,
                        );
                    if creep_count < target_count {
                        let creep_name =
                            format!("{}-{}-{}", spawn_goal.name, game::time(), additional);
                        let room = spawn.room().unwrap();
                        let body_cost = spawn_goal.body.iter().map(|p| p.cost()).sum::<u32>();
                        let additive_parts_cost = spawn_goal
                            .body_upgrades
                            .iter()
                            .map(|p| p.cost())
                            .sum::<u32>()
                            + 1;
                        let mut body_parts = spawn_goal.body.clone();

                        if room.energy_available() >= body_cost {
                            if !spawn_goal.body_upgrades.is_empty() {
                                let remaining_energy =
                                    std::cmp::max(room.energy_available() - body_cost, 0);
                                let times_to_add = std::cmp::min(
                                    remaining_energy / additive_parts_cost,
                                    spawn_goal.max_body_upgrades,
                                );
                                info!(
                                    "Upgrading the {} creep {} times for an additional {} energy",
                                    spawn_goal.name,
                                    times_to_add,
                                    times_to_add * (additive_parts_cost - 1)
                                );
                                for _ in 0..times_to_add {
                                    for part in spawn_goal.body_upgrades.iter() {
                                        body_parts.push(*part);
                                    }
                                }
                            }

                            info!(
                                "Spawning {} [{}/{}]",
                                spawn_goal.name, creep_count, target_count
                            );

                            match spawn.spawn_creep(&body_parts, &creep_name) {
                                Ok(()) => {
                                    additional += 1;
                                    self.room_creep_counts
                                        .get_mut(&room_name)
                                        .unwrap()
                                        .insert(spawn_goal.name.clone(), creep_count + 1);
                                }
                                Err(e) => debug!("couldn't spawn {}: {:?}", spawn_goal.name, e),
                            }

                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn get_creep_count_in_room(&self, room_name: &RoomName, creep_type: &str) -> u32 {
        let creep_counts = self.room_creep_counts.get(room_name);
        if let Some(creep_counts) = creep_counts {
            if let Some(creep_count) = creep_counts.get(creep_type) {
                return *creep_count;
            }
        }
        0
    }

    pub fn get_global_creep_count(&self, creep_type: &str) -> u32 {
        let mut count = 0;
        for room_counts in self.room_creep_counts.values() {
            if let Some(creep_count) = room_counts.get(creep_type) {
                count += creep_count;
            }
        }
        count
    }
}
