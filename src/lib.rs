use std::cell::RefCell;

use log::*;
use screeps::{constants::Part, enums::StructureObject, find, game};
use screeps::{HasPosition, Room, RoomObjectProperties, StructureProperties, StructureType};
use spawn::{SpawnGoal, SpawnGoals, SpawnManager};
use tasks::TaskManager;
use wasm_bindgen::prelude::*;

mod logging;
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
}

// to use a reserved name as a function name, use `js_name`:
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    debug!(
        "loop starting! CPU: {}. Peak Malloc: {}. Total Memory: {}",
        game::cpu::get_used(),
        game::cpu::get_heap_statistics().peak_malloced_memory(),
        game::cpu::get_heap_statistics().total_heap_size()
    );

    TASK_MANAGER.with(|task_manager_refcell| {
        let mut task_manager = task_manager_refcell.borrow_mut();
        task_manager.clean_up_tasks();
        let flag_tasks = task_manager.assign_tasks();
        task_manager.execute_tasks();

        let claim_task_exists = flag_tasks
            .iter()
            .any(|t| t.get_type() == tasks::TaskType::Claim);

        let spawn_goals: SpawnGoals = vec![
            SpawnGoal {
                name: "worker".to_string(),
                body: vec![Part::Work, Part::Carry, Part::Move, Part::Move],
                additive_body: vec![Part::Work, Part::Carry, Part::Move],
                max_additions: 5,
                source_modifier: 1,
                count: 4,
                is_global: false,
            },
            SpawnGoal {
                name: "melee".to_string(),
                body: vec![Part::Move, Part::Attack, Part::Attack],
                additive_body: vec![],
                max_additions: 0,
                count: 2,
                source_modifier: 0,
                is_global: false,
            },
            SpawnGoal {
                name: "claimer".to_string(),
                body: vec![Part::Claim, Part::Move],
                additive_body: vec![],
                max_additions: 0,
                source_modifier: 0,
                count: if claim_task_exists { 1 } else { 0 },
                is_global: true,
            },
        ];

        SpawnManager::new(spawn_goals).spawn_creeps();
    });

    let creeps = game::creeps().values().collect::<Vec<_>>();

    if !creeps.is_empty() {
        execute_towers(&creeps.get(0).unwrap().room().unwrap());
    }

    info!(
        "Done! cpu: {} Peak Malloc: {}. Total Memory: {}",
        game::cpu::get_used(),
        game::cpu::get_heap_statistics().peak_malloced_memory(),
        game::cpu::get_heap_statistics().total_heap_size()
    );
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
