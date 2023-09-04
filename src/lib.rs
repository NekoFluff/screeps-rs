use std::{cell::RefCell, collections::HashMap};

use log::*;
use screeps::{
    constants::{Part, ResourceType},
    enums::StructureObject,
    find, game,
    objects::Creep,
    prelude::*,
    RoomName, RoomPosition,
};
use screeps::{Room, RoomObjectProperties, StructureType};
use spawn::{SpawnGoal, SpawnGoals, SpawnManager};
use tasks::{
    AttackTask, BuildTask, ClaimTask, HarvestTask, RepairTask, Task, TaskManager, TransferTask,
    TravelTask, UpgradeTask,
};
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
        let spawn_goals: SpawnGoals = vec![
            SpawnGoal {
                name: "worker".to_string(),
                body: vec![Part::Work, Part::Carry, Part::Move],
                additive_body: vec![Part::Work, Part::Carry, Part::Move],
                count: 5,
                is_global: false,
            },
            SpawnGoal {
                name: "melee".to_string(),
                body: vec![Part::Move, Part::Attack, Part::Attack],
                additive_body: vec![],
                count: 2,
                is_global: false,
            },
            SpawnGoal {
                name: "claimer".to_string(),
                body: vec![Part::Claim, Part::Move],
                additive_body: vec![],
                count: 1,
                is_global: true,
            },
        ];

        SpawnManager::new(spawn_goals).spawn_creeps();

        let mut task_manager = task_manager_refcell.borrow_mut();

        task_manager.assign_tasks();

        task_manager.execute_tasks();
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
    let structures = room.find(find::MY_STRUCTURES, None);

    let towers = structures
        .iter()
        .filter(|s| s.structure_type() == StructureType::Tower);

    // get the closest enemies to each tower
    for tower in towers {
        let mut enemies = room.find(find::HOSTILE_CREEPS, None);

        if enemies.is_empty() {
            continue;
        }

        enemies.sort_by(|a, b| {
            tower
                .pos()
                .get_range_to(a.pos())
                .cmp(&tower.pos().get_range_to(b.pos()))
        });

        let enemy = enemies.first().unwrap();

        if let StructureObject::StructureTower(tower) = tower {
            let _ = tower.attack(enemy);
        }
    }
}
