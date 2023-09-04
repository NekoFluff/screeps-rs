use std::cell::RefCell;
use std::collections::hash_map::Entry;

use log::*;
use screeps::{
    constants::{Part, ResourceType},
    enums::StructureObject,
    find, game,
    objects::Creep,
    prelude::*,
};
use screeps::{Room, RoomObjectProperties, StructureType};
use spawn::{SpawnGoal, SpawnGoals, SpawnManager};
use tasks::{
    AttackTask, BuildTask, HarvestTask, RepairTask, Task, TaskManager, TransferTask, TravelTask,
    UpgradeTask,
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
        let mut task_manager = task_manager_refcell.borrow_mut();
        debug!("running creeps");

        let creeps = game::creeps().values();
        let idle_creeps = creeps
            .filter(|c| match task_manager.tasks.entry(c.try_id().unwrap()) {
                Entry::Occupied(_) => false,
                Entry::Vacant(_) => true,
            })
            .collect::<Vec<Creep>>();
        let idle_creep_count = idle_creeps.len();

        let mut tasks = Vec::new();
        if let Some(creep) = idle_creeps.get(0) {
            tasks = get_potential_creep_tasks(&creep.room().unwrap(), idle_creep_count);
        }

        for creep in idle_creeps {
            if let Some(task) = get_task_for_creep(&creep, &mut tasks) {
                task_manager.add_task(&creep, task);
            }
        }

        task_manager.execute_tasks();
    });

    let spawn_goals: SpawnGoals = vec![
        SpawnGoal {
            name: "worker".to_string(),
            body: vec![Part::Work, Part::Carry, Part::Move],
            additive_body: vec![Part::Work, Part::Carry, Part::Move],
            count: 5,
        },
        SpawnGoal {
            name: "melee".to_string(),
            body: vec![Part::Move, Part::Attack, Part::Attack],
            additive_body: vec![],
            count: 2,
        },
    ];

    SpawnManager::new(spawn_goals).spawn_creeps();

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

fn get_potential_creep_tasks(room: &Room, max_tasks: usize) -> Vec<Box<dyn Task>> {
    let mut creep_targets: Vec<Box<dyn Task>> = Vec::new();

    let structures = room.find(find::STRUCTURES, None);
    let construction_sites = room.find(find::CONSTRUCTION_SITES, None);
    let controller = room.controller();
    let enemy_creeps = room.find(find::HOSTILE_CREEPS, None);

    // attack
    if !enemy_creeps.is_empty() {
        for enemy_creep in enemy_creeps {
            if let Some(id) = enemy_creep.try_id() {
                creep_targets.push(Box::new(AttackTask::new(id)));
                creep_targets.push(Box::new(AttackTask::new(id)));
                if creep_targets.len() >= max_tasks {
                    return creep_targets;
                }
            }
        }
    }

    // controller: if the downgrade time is less than 10000 ticks, upgrade
    if let Some(c) = controller {
        if let Some(owner) = c.owner() {
            if owner.username() == "CrazyFluff" && c.ticks_to_downgrade() < 10000 && c.is_active() {
                creep_targets.push(Box::new(UpgradeTask::new(c.id())));
                if creep_targets.len() >= max_tasks {
                    return creep_targets;
                }
            }
        }
    }

    // towers
    let towers = structures
        .iter()
        .filter(|s| s.structure_type() == StructureType::Tower);
    for tower in towers {
        if let StructureObject::StructureTower(tower) = tower {
            if tower.is_active() && tower.store().get_free_capacity(Some(ResourceType::Energy)) > 0
            {
                if let Some(id) = tower.try_id() {
                    creep_targets.push(Box::new(TransferTask::new(id)));
                    if creep_targets.len() >= max_tasks {
                        return creep_targets;
                    }
                }
            }
        }
    }

    // extensions
    let extensions = structures
        .iter()
        .filter(|s| s.structure_type() == StructureType::Extension);
    for extension in extensions {
        if let StructureObject::StructureExtension(extension) = extension {
            if extension.is_active()
                && extension
                    .store()
                    .get_free_capacity(Some(ResourceType::Energy))
                    > 0
                && extension.owner().unwrap().username() == "CrazyFluff"
            {
                if let Some(id) = extension.try_id() {
                    creep_targets.push(Box::new(TransferTask::new(id)));
                }
            }
        }
    }

    if creep_targets.len() >= max_tasks {
        return creep_targets;
    }

    // spawn
    if let Some(spawn) = room.find(find::MY_SPAWNS, None).get(0) {
        if spawn.is_active() && spawn.store().get_free_capacity(Some(ResourceType::Energy)) > 0 {
            if let Some(id) = spawn.try_id() {
                creep_targets.push(Box::new(TransferTask::new(id)));
                creep_targets.push(Box::new(TransferTask::new(id)));
                creep_targets.push(Box::new(TransferTask::new(id)));

                if creep_targets.len() >= max_tasks {
                    return creep_targets;
                }
            }
        }
    }

    // healing
    // if creep.hits() < creep.hits_max() {
    //     info!("{} needs healing", creep.name());
    //     entry.insert(CreepTarget::Heal(creep.try_id().unwrap()));
    //     return;
    // }

    // repair
    for structure in structures.iter() {
        let s = structure.as_structure();
        if s.hits() < s.hits_max() / 2 {
            if let StructureObject::StructureWall(wall) = structure {
                if wall.hits() > 25000 {
                    continue;
                }
            } else if let StructureObject::StructureRoad(road) = structure {
                if road.hits() > 100 {
                    continue;
                }
            } else if let StructureObject::StructureRampart(road) = structure {
                if road.hits() > 100000 {
                    continue;
                }
            }
            let id = s.try_id().unwrap();
            creep_targets.push(Box::new(RepairTask::new(id)));
            if creep_targets.len() >= max_tasks {
                return creep_targets;
            }
        }
    }

    // construction sites
    for construction_site in construction_sites.iter() {
        if let Some(id) = construction_site.try_id() {
            creep_targets.push(Box::new(BuildTask::new(id)));
            if creep_targets.len() >= max_tasks {
                return creep_targets;
            }
        }
    }

    creep_targets
}

pub fn get_task_for_creep(
    creep: &Creep,
    task_list: &mut Vec<Box<dyn Task>>,
) -> Option<Box<dyn Task>> {
    let creep_parts = creep.body().iter().map(|p| p.part()).collect::<Vec<Part>>();

    if creep_parts.contains(&Part::Work)
        && creep.store().get_used_capacity(Some(ResourceType::Energy)) == 0
    {
        if let Some(source) = creep
            .room()
            .unwrap()
            .find(find::SOURCES_ACTIVE, None)
            .get(0)
        {
            return Some(Box::new(HarvestTask::new(source.id())));
        }
    }

    // (index, task)
    let mut similar_tasks: Vec<(usize, &Box<dyn Task>)> = vec![];
    for (index, task) in task_list.iter().enumerate() {
        if similar_tasks.is_empty()
            && task
                .requires_body_parts()
                .iter()
                .all(|p| creep_parts.contains(p))
        {
            error!("found task: {:?}", task);
            similar_tasks.push((index, task));
            continue;
        } else if !similar_tasks.is_empty() {
            let first_task = similar_tasks.get(0).unwrap().1;
            if task.get_type() == first_task.get_type() {
                similar_tasks.push((index, task));
            } else {
                break;
            }
        }
    }

    // Default task
    if similar_tasks.is_empty() {
        if creep_parts.contains(&Part::Attack) {
            let controller = creep.room().unwrap().controller().unwrap();
            if !creep.pos().in_range_to(controller.pos(), 3) {
                return Some(Box::new(TravelTask::new(controller.id())));
            } else {
                return None;
            }
        } else {
            return Some(Box::new(UpgradeTask::new(
                creep.room().unwrap().controller().unwrap().id(),
            )));
        }
    }
    // info!("similar tasks: {:?}", similar_tasks);

    if similar_tasks.len() == 1 {
        return Some(task_list.remove(similar_tasks.get(0).unwrap().0));
    }

    // (index, distance to target)
    let mut tasks_by_distance = similar_tasks
        .iter()
        .map(|t| {
            if let Some(target) = t.1.get_target_pos() {
                let distance = creep.pos().get_range_to(target);
                return (t.0, distance);
            }
            (t.0, u32::MAX)
        })
        .collect::<Vec<(usize, u32)>>();

    tasks_by_distance.sort_by(|a, b| a.1.cmp(&b.1));
    // info!("sorted tasks: {:?}", tasks_by_distance);

    let shortest_distance_idx = tasks_by_distance.first().unwrap().0;

    Some(task_list.remove(shortest_distance_idx))
}
