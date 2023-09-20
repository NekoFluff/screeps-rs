use std::cell::RefCell;
use std::collections::HashMap;

use log::*;
use screeps::{constants::Part, enums::StructureObject, find, game};
use screeps::{HasPosition, ResourceType, RoomName, StructureProperties, StructureType};
use spawn::{SpawnGoal, SpawnGoals, SpawnManager};
use tasks::TaskManager;
use wasm_bindgen::prelude::*;

mod logging;
mod metadata;
mod spawn;
mod tasks;
mod utils;

// add wasm_bindgen to any function you would like to expose for call from js
#[wasm_bindgen]
pub fn setup() {
    logging::setup_logging(logging::Info);
}

// this is one way to persist data between ticks within Rust's memory, as opposed to
// keeping state in memory on game objects - but will be lost on global resets!
thread_local! {
    static TASK_MANAGER: RefCell<TaskManager> = RefCell::new(TaskManager::new());
    static SOURCE_DATA: RefCell<Vec<metadata::SourceInfo>> = RefCell::new(Vec::new());
    static PAUSE_SCRIPT: RefCell<bool> = RefCell::new(false);
    static LAST_CPU_USAGE: RefCell<f64> = RefCell::new(0_f64);
    static AVERAGE_CPU_USAGE_X_TICKS: RefCell<Vec<f64>> = RefCell::new(Vec::new());
}

// to use a reserved name as a function name, use `js_name`:
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    let pause = PAUSE_SCRIPT.with(|p| *p.borrow());
    if pause {
        return;
    }
    LAST_CPU_USAGE.with(|l| {
        *l.borrow_mut() = screeps::game::cpu::get_used();
    });

    debug!(
        "loop starting! CPU: {}. Peak Malloc: {}. Total Memory: {}",
        game::cpu::get_used(),
        game::cpu::get_heap_statistics().peak_malloced_memory(),
        game::cpu::get_heap_statistics().total_heap_size()
    );

    TASK_MANAGER.with(|task_manager_refcell| {
        let rooms = game::rooms().values();
        utils::log_cpu_usage("get rooms");

        let mut task_manager = task_manager_refcell.borrow_mut();
        task_manager.clean_up_tasks();
        utils::log_cpu_usage("clean up tasks");
        task_manager.refresh_room_info();
        utils::log_cpu_usage("refresh room info");
        let flag_tasks_lists = task_manager.assign_tasks();
        utils::log_cpu_usage("assign tasks");
        task_manager.execute_tasks();
        utils::log_cpu_usage("execute tasks");

        for room in rooms {
            execute_towers(task_manager.room_info_map.get(&room.name()).unwrap());
            utils::log_cpu_usage(stringify!("execute towers in room {}", room.name()));
        }

        let claim_task_exists = flag_tasks_lists.iter().any(|t| {
            if let Some(task) = t.current_task() {
                task.get_type() == tasks::TaskType::Claim
            } else {
                false
            }
        });

        // Spawn creeps
        let mut room_spawn_goals: HashMap<RoomName, SpawnGoals> = HashMap::new();
        for room in game::rooms().values() {
            let spawns = room.find(find::MY_SPAWNS, None);
            let spawn = spawns.first();
            if spawn.is_none() {
                continue;
            }

            let room_name = room.name();
            let spawn_goals = room_spawn_goals.entry(room_name).or_default();

            let sources = room.find(find::SOURCES, None);
            let source_infos = sources
                .iter()
                .map(|s| metadata::SourceInfo::new(s, None))
                .collect::<Vec<_>>();

            let link_type_map = &task_manager.room_info_map.get(&room.name()).unwrap().links;

            let source_link_has_output = !(link_type_map.storage_links.is_empty()
                && link_type_map.controller_links.is_empty());

            let target_worker_count = source_infos
                .iter()
                .map(|s| {
                    if !s.has_link || !source_link_has_output {
                        s.non_wall_terrain_count
                    } else {
                        0
                    }
                })
                .sum::<u32>()
                + 1;

            let source_link_count = link_type_map.source_links.len();

            spawn_goals.push(SpawnGoal {
                name: "source_harvester".to_string(),
                body: vec![
                    Part::Move,
                    Part::Move,
                    Part::Move,
                    Part::Carry,
                    Part::Carry,
                    Part::Carry,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                ],
                body_upgrades: vec![],
                max_body_upgrades: 0,
                source_modifier: 0,
                count: if source_link_has_output {
                    source_link_count as u32
                } else {
                    0
                },
                is_global: false,
            });

            let controller_link_count = link_type_map.controller_links.len();
            let controller_link_energy = link_type_map
                .controller_links
                .iter()
                .map(|metadata::ControllerLink(l, _)| {
                    l.store().get_used_capacity(Some(ResourceType::Energy))
                })
                .sum::<u32>();
            let source_link_count = link_type_map.source_links.len();

            let mut body = vec![
                Part::Move,
                Part::Move,
                Part::Carry,
                Part::Carry,
                Part::Carry,
                Part::Carry,
            ];
            for _ in 0..source_link_count {
                body.append(&mut vec![
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                    Part::Work,
                ]);
            }
            spawn_goals.push(SpawnGoal {
                name: "upgrader".to_string(),
                body,
                body_upgrades: vec![],
                max_body_upgrades: 0,
                source_modifier: 0,
                count: if source_link_count > 0 && controller_link_energy > 400 {
                    controller_link_count as u32
                } else {
                    0
                },
                is_global: false,
            });

            let storage_link_count = link_type_map.storage_links.len();
            let storage_link_energy = link_type_map
                .storage_links
                .iter()
                .map(|metadata::StorageLink(l, _)| {
                    l.store().get_used_capacity(Some(ResourceType::Energy))
                })
                .sum::<u32>();
            spawn_goals.push(SpawnGoal {
                name: "storager".to_string(),
                body: vec![
                    Part::Move,
                    Part::Move,
                    Part::Carry,
                    Part::Carry,
                    Part::Carry,
                    Part::Carry,
                    Part::Work,
                ],
                body_upgrades: vec![],
                max_body_upgrades: 0,
                source_modifier: 0,
                count: if source_link_count > 0 && storage_link_energy > 400 {
                    storage_link_count as u32
                } else {
                    0
                },
                is_global: false,
            });

            spawn_goals.push(SpawnGoal {
                name: "worker".to_string(),
                body: vec![Part::Move, Part::Move, Part::Carry, Part::Work],
                body_upgrades: vec![Part::Move, Part::Carry, Part::Work],
                max_body_upgrades: 4,
                source_modifier: 0,
                count: std::cmp::min(target_worker_count, source_infos.len() as u32 * 4),
                is_global: false,
            });

            spawn_goals.push(SpawnGoal {
                name: "melee".to_string(),
                body: vec![Part::Move, Part::Move, Part::Attack, Part::Attack],
                body_upgrades: vec![],
                max_body_upgrades: 0,
                count: 0,
                source_modifier: 0,
                is_global: true, // TODO: Fix defend flag mechanic
            });

            spawn_goals.push(SpawnGoal {
                name: "claimer".to_string(),
                body: vec![Part::Move, Part::Claim],
                body_upgrades: vec![],
                max_body_upgrades: 0,
                source_modifier: 0,
                count: if claim_task_exists { 1 } else { 0 },
                is_global: true,
            });

            // info!("spawn goals for room {}: {:?}", room_name, spawn_goals);
        }
        utils::log_cpu_usage("calculate spawn goals");
        SpawnManager::new(room_spawn_goals).spawn_creeps();
        utils::log_cpu_usage("spawn creeps");
    });

    info!(
        "Done! cpu: {} Peak Malloc: {}. Total Memory: {}",
        game::cpu::get_used(),
        game::cpu::get_heap_statistics().peak_malloced_memory(),
        game::cpu::get_heap_statistics().total_heap_size()
    );

    // update average cpu usage
    let max_ticks = 1000;
    AVERAGE_CPU_USAGE_X_TICKS.with(|a| {
        let mut average_cpu_usage = a.borrow_mut();
        let cpu = game::cpu::get_used();
        if cpu > 20_f64 {
            return;
        }
        average_cpu_usage.push(cpu);
        if average_cpu_usage.len() > max_ticks {
            average_cpu_usage.remove(0);
        }
    });

    let len = AVERAGE_CPU_USAGE_X_TICKS.with(|a| a.borrow().len()) as f64;
    let min =
        AVERAGE_CPU_USAGE_X_TICKS.with(|a| a.borrow().iter().copied().fold(f64::NAN, f64::min));
    let max =
        AVERAGE_CPU_USAGE_X_TICKS.with(|a| a.borrow().iter().copied().fold(f64::NAN, f64::max));
    info!(
        "[CPU usage] [Avg: {:.2} | Min: {:.2} | Max: {:.2}] [{} ticks] | Final {:.2}",
        AVERAGE_CPU_USAGE_X_TICKS.with(|a| { a.borrow().iter().sum::<f64>() / len }),
        min,
        max,
        len,
        game::cpu::get_used()
    );
}

fn execute_towers(room_info: &metadata::RoomInfo) {
    let structures = &room_info.structures;
    let my_structures = &room_info.my_structures;
    let mut enemies = room_info.room.find(find::HOSTILE_CREEPS, None);

    let towers = my_structures
        .iter()
        .filter(|s| s.structure_type() == StructureType::Tower);

    // get injured creeps
    let creeps = game::creeps().values().collect::<Vec<_>>();
    let mut injured = creeps
        .iter()
        .filter(|c| c.hits() < c.hits_max() && c.my())
        .collect::<Vec<_>>();
    injured.sort_by_key(|a| a.hits());

    // get damaged structures (anything with less than 100K hit points)
    let mut damaged = structures
        .iter()
        .map(|s| s.as_structure())
        .filter(|s| {
            let x = (s.hits() as f32 / s.hits_max() as f32) < 0.8;
            let y = s.hits() < 100000;
            let z = s.structure_type() != StructureType::Wall;
            x && y && z
        })
        .collect::<Vec<_>>();

    damaged.sort_by_key(|a| a.hits());

    for tower in towers {
        // attack the closest enemy creep
        enemies.sort_by(|a, b| {
            tower
                .pos()
                .get_range_to(a.pos())
                .cmp(&tower.pos().get_range_to(b.pos()))
        });

        if let Some(enemy) = enemies.first() {
            if let StructureObject::StructureTower(tower) = tower {
                let _ = tower.attack(enemy);
                continue;
            }
        }

        if let Some(creep) = injured.first() {
            if let StructureObject::StructureTower(tower) = tower {
                tower
                    .heal(*creep)
                    .unwrap_or_else(|e| info!("couldn't heal: {:?}", e));
                continue;
            }
        }

        if let Some(structure) = damaged.first() {
            if let StructureObject::StructureTower(tower) = tower {
                let _ = tower.repair(structure);
                continue;
            }
        }
    }
}
