use std::collections::hash_map::Entry;
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

        let mut flag_tasks = get_flag_tasks();
        let mut room_tasks_map = HashMap::new();
        for room in game::rooms().values() {
            room_tasks_map.insert(
                room.name(),
                get_potential_creep_tasks(room, idle_creep_count * 2),
            );
        }

        for creep in idle_creeps {
            if let Some(task) = get_task_for_creep(&creep, &mut flag_tasks) {
                task_manager.add_task(&creep, task);
                continue;
            }

            let room_tasks = room_tasks_map
                .get_mut(&creep.room().unwrap().name())
                .unwrap();
            if let Some(task) = get_task_for_creep(&creep, room_tasks) {
                task_manager.add_task(&creep, task);
                continue;
            }

            for (room_name, room_tasks) in room_tasks_map.iter_mut() {
                if room_name == &creep.room().unwrap().name() {
                    continue;
                }

                if let Some(task) = get_task_for_creep(&creep, room_tasks) {
                    task_manager.add_task(&creep, task);
                    continue;
                }
            }

            if let Some(task) = get_default_task_for_creep(&creep) {
                task_manager.add_task(&creep, task)
            }
        }

        task_manager.execute_tasks();

        let has_claim_task = flag_tasks
            .iter()
            .any(|t| t.get_type() == tasks::TaskType::Claim);
        let spawn_goals: SpawnGoals = vec![
            SpawnGoal {
                name: "worker".to_string(),
                body: vec![Part::Work, Part::Carry, Part::Move],
                additive_body: vec![Part::Work, Part::Carry, Part::Move],
                count: 6,
            },
            SpawnGoal {
                name: "melee".to_string(),
                body: vec![Part::Move, Part::Attack, Part::Attack],
                additive_body: vec![],
                count: 2,
            },
            SpawnGoal {
                name: "claimer".to_string(),
                body: vec![Part::Claim, Part::Move],
                additive_body: vec![],
                count: if has_claim_task { 1 } else { 0 },
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

fn get_flag_tasks() -> Vec<Box<dyn Task>> {
    let mut tasks: Vec<Box<dyn Task>> = Vec::new();
    let flags = game::flags().values();

    for flag in flags {
        if flag.name().starts_with("claim", 0) {
            let room_name = flag
                .name()
                .split(":")
                .pop()
                .as_string()
                .unwrap_or("".to_string());
            info!("claiming room v1 {}", room_name);

            if let Ok(room_name) = RoomName::new(&room_name) {
                // if the room is already owned, remove the flag
                if let Some(room) = game::rooms().get(room_name) {
                    if let Some(controller) = room.controller() {
                        if controller.owner().is_some() || controller.reservation().is_some() {
                            error!("room {} is already owned or reserved", room_name);
                            flag.remove();
                            continue;
                        }
                    }
                }

                info!("claiming room v2 {}", room_name);

                let room_pos = RoomPosition::new(25, 25, room_name);
                tasks.push(Box::new(ClaimTask::new(room_pos)));
            } else {
                error!("invalid room name: {}", room_name);
                flag.remove();
            }
        }
    }

    tasks
}

fn get_potential_creep_tasks(room: Room, max_tasks: usize) -> Vec<Box<dyn Task>> {
    let mut tasks: Vec<Box<dyn Task>> = Vec::new();

    let structures = room.find(find::STRUCTURES, None);
    let construction_sites = room.find(find::CONSTRUCTION_SITES, None);
    let controller = room.controller().unwrap();
    let enemy_creeps = room.find(find::HOSTILE_CREEPS, None);

    // attack
    if !enemy_creeps.is_empty() {
        for enemy_creep in enemy_creeps {
            if let Some(id) = enemy_creep.try_id() {
                tasks.push(Box::new(AttackTask::new(id)));
                tasks.push(Box::new(AttackTask::new(id)));
            }
        }
    }

    // controller: if the downgrade time is less than 10000 ticks, upgrade
    if let Some(owner) = controller.owner() {
        if owner.username() == "CrazyFluff"
            && controller.ticks_to_downgrade() < 9000
            && controller.is_active()
        {
            tasks.push(Box::new(UpgradeTask::new(controller.id())));
            if tasks.len() >= max_tasks {
                return tasks;
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
                    tasks.push(Box::new(TransferTask::new(id)));
                    if tasks.len() >= max_tasks {
                        return tasks;
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
                    tasks.push(Box::new(TransferTask::new(id)));
                }
            }
        }
    }

    if tasks.len() >= max_tasks {
        return tasks;
    }

    // spawn
    let spawns = structures
        .iter()
        .filter(|s| s.structure_type() == StructureType::Spawn);

    for spawn in spawns {
        if let StructureObject::StructureSpawn(spawn) = spawn {
            if spawn.is_active() && spawn.store().get_free_capacity(Some(ResourceType::Energy)) > 0
            {
                if let Some(id) = spawn.try_id() {
                    tasks.push(Box::new(TransferTask::new(id)));

                    if tasks.len() >= max_tasks {
                        return tasks;
                    }
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

    // construction sites
    for construction_site in construction_sites.iter() {
        if let Some(id) = construction_site.try_id() {
            tasks.push(Box::new(BuildTask::new(id)));
            if tasks.len() >= max_tasks {
                return tasks;
            }
        }
    }

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
            tasks.push(Box::new(RepairTask::new(id)));
            if tasks.len() >= max_tasks {
                return tasks;
            }
        }
    }

    tasks
}

pub fn get_task_for_creep(
    creep: &Creep,
    task_list: &mut Vec<Box<dyn Task>>,
) -> Option<Box<dyn Task>> {
    let creep_parts = creep.body().iter().map(|p| p.part()).collect::<Vec<Part>>();
    let room = creep.room().unwrap();

    if creep_parts.contains(&Part::Work)
        && creep.store().get_used_capacity(Some(ResourceType::Energy)) == 0
    {
        // Gather energy
        if let Some(controller) = room.controller() {
            if let Some(owner) = controller.owner() {
                if owner.username() == creep.owner().username() {
                    let mut sources = room.find(find::SOURCES_ACTIVE, None);
                    sources.sort_by_key(|a| 0 - a.energy());

                    if let Some(source) = sources.first() {
                        return Some(Box::new(HarvestTask::new(source.id())));
                    }
                }
            }
        }

        // Go back to an owned room if we can't harvest in the current room
        let rooms = screeps::game::rooms().values();
        let my_owned_rooms = rooms
            .filter(|room| {
                room.controller()
                    .map(|c| {
                        c.owner()
                            .is_some_and(|o| o.username() == creep.owner().username())
                    })
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        if let Some(room) = my_owned_rooms.first() {
            return Some(Box::new(TravelTask::new(room.controller().unwrap().id())));
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
        return None;
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

fn get_default_task_for_creep(creep: &Creep) -> Option<Box<dyn Task>> {
    let creep_parts = creep.body().iter().map(|p| p.part()).collect::<Vec<Part>>();

    if creep_parts.contains(&Part::Attack) {
        let controller = creep.room().unwrap().controller().unwrap();
        if !creep.pos().in_range_to(controller.pos(), 3) {
            Some(Box::new(TravelTask::new(controller.id())))
        } else {
            None
        }
    } else if creep_parts.contains(&Part::Claim) {
        return None;
    } else {
        return Some(Box::new(UpgradeTask::new(
            creep.room().unwrap().controller().unwrap().id(),
        )));
    }
}
