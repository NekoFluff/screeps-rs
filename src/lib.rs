use std::cell::RefCell;
use std::collections::HashMap;

use log::*;
use screeps::{constants::Part, enums::StructureObject, find, game};
use screeps::{HasPosition, Room, RoomName, StructureProperties, StructureType};
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
}

// to use a reserved name as a function name, use `js_name`:
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    let pause = PAUSE_SCRIPT.with(|p| *p.borrow());
    if pause {
        return;
    }

    debug!(
        "loop starting! CPU: {}. Peak Malloc: {}. Total Memory: {}",
        game::cpu::get_used(),
        game::cpu::get_heap_statistics().peak_malloced_memory(),
        game::cpu::get_heap_statistics().total_heap_size()
    );

    TASK_MANAGER.with(|task_manager_refcell| {
        let rooms = game::rooms().values();

        for room in rooms {
            execute_towers(&room);
        }

        let mut task_manager = task_manager_refcell.borrow_mut();
        task_manager.clean_up_tasks();
        task_manager.classify_links();
        let flag_tasks = task_manager.assign_tasks();
        task_manager.execute_tasks();

        let claim_task_exists = flag_tasks
            .iter()
            .any(|t| t.get_type() == tasks::TaskType::Claim);

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
                .map(metadata::SourceInfo::new)
                .collect::<Vec<_>>();

            spawn_goals.push(SpawnGoal {
                name: "worker".to_string(),
                body: vec![Part::Move, Part::Move, Part::Carry, Part::Work],
                body_upgrades: vec![Part::Move, Part::Carry, Part::Work],
                max_body_upgrades: 5,
                source_modifier: 0,
                count: std::cmp::min(
                    source_infos
                        .iter()
                        .map(|s| s.non_wall_terrain_count)
                        .sum::<u32>()
                        + 2,
                    source_infos.len() as u32 * 4,
                ),
                is_global: false,
            });

            spawn_goals.push(SpawnGoal {
                name: "melee".to_string(),
                body: vec![Part::Move, Part::Move, Part::Attack, Part::Attack],
                body_upgrades: vec![],
                max_body_upgrades: 0,
                count: 2,
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
        SpawnManager::new(room_spawn_goals).spawn_creeps();
    });

    info!(
        "Done! cpu: {} Peak Malloc: {}. Total Memory: {}",
        game::cpu::get_used(),
        game::cpu::get_heap_statistics().peak_malloced_memory(),
        game::cpu::get_heap_statistics().total_heap_size()
    );
}

struct LinkTypeMap {
    source_links: Vec<StructureObject>,
    storage_links: Vec<StructureObject>,
    controller_links: Vec<StructureObject>,
    unknown_links: Vec<StructureObject>,
}

impl LinkTypeMap {
    fn new() -> LinkTypeMap {
        LinkTypeMap {
            source_links: Vec::new(),
            storage_links: Vec::new(),
            controller_links: Vec::new(),
            unknown_links: Vec::new(),
        }
    }
}

fn execute_towers(room: &Room) {
    let structures = room.find(find::STRUCTURES, None);
    let my_structures = room.find(find::MY_STRUCTURES, None);
    let mut enemies = room.find(find::HOSTILE_CREEPS, None);

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
