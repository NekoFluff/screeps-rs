use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

use log::*;
use screeps::{
    find, game, Creep, HasHits, HasPosition, HasTypedId, MaybeHasTypedId, ObjectId,
    OwnedStructureProperties, Part, ResourceType, Room, RoomName, RoomPosition,
    SharedCreepProperties, StructureObject, StructureProperties, StructureType,
};

mod attack;
mod build;
mod claim;
mod harvest;
mod heal;
mod repair;
mod transfer;
mod travel;
mod upgrade;

pub use attack::AttackTask;
pub use build::BuildTask;
pub use claim::ClaimTask;
pub use harvest::HarvestTask;
pub use heal::HealTask;
pub use repair::RepairTask;
pub use transfer::TransferTask;
pub use travel::TravelTask;
pub use upgrade::UpgradeTask;
use wasm_bindgen::JsValue;

use crate::utils::{self, get_creep_type};

pub struct TaskManager {
    pub tasks: HashMap<ObjectId<Creep>, Box<dyn Task>>,
    working_creeps_by_room: HashMap<RoomName, HashMap<String, u32>>,
}

impl TaskManager {
    pub fn new() -> TaskManager {
        TaskManager {
            tasks: HashMap::new(),
            working_creeps_by_room: HashMap::new(),
        }
    }

    fn recalculate_working_creeps(&mut self) {
        let tasks = self.tasks.iter();
        self.working_creeps_by_room = tasks.fold(HashMap::new(), |mut acc, (creep_id, task)| {
            let creep = game::get_object_by_id_typed(creep_id);
            if creep.is_none() {
                return acc;
            }

            let creep = creep.unwrap();
            let creep_type = get_creep_type(&creep);
            let room_name = task
                .get_target_pos()
                .map(|p| p.room_name())
                .unwrap_or(creep.room().unwrap().name());

            let count: &mut HashMap<String, u32> = acc.entry(room_name).or_default();
            let creep_count = count.entry(creep_type).or_insert(0);
            *creep_count += 1;
            acc
        });
    }

    pub fn add_task(&mut self, creep: &Creep, task: Box<dyn Task>) {
        if let Some(creep_id) = creep.try_id() {
            info!("{} was assigned to {:?}", creep.name(), task);
            let _ = js_sys::Reflect::set(
                &creep.memory(),
                &JsValue::from_str("task"),
                &JsValue::from_str(&format!("{:?}", task)),
            );
            self.tasks.insert(creep_id, task);
        }
    }

    pub fn execute_tasks(&mut self) {
        type TaskMap = HashMap<ObjectId<Creep>, Box<dyn Task>>;

        let completed_tasks = Rc::new(RefCell::new(Vec::new()));
        let cancelled_tasks = Rc::new(RefCell::new(Vec::new()));
        let switch_tasks: Rc<RefCell<TaskMap>> = Rc::new(RefCell::new(HashMap::new()));

        for (creep_id, task) in self.tasks.iter() {
            if let Some(creep) = game::get_object_by_id_typed(creep_id) {
                let completed_tasks_clone = completed_tasks.clone();
                let cancelled_tasks_clone = cancelled_tasks.clone();
                let switch_tasks_clone = switch_tasks.clone();
                task.execute(
                    &creep,
                    Box::new(move |creep_id| completed_tasks_clone.borrow_mut().push(creep_id)),
                    Box::new(move |creep_id| cancelled_tasks_clone.borrow_mut().push(creep_id)),
                    Box::new(move |creep_id, task| {
                        switch_tasks_clone.borrow_mut().insert(creep_id, task);
                    }),
                );
            }
        }
        for completed_task in completed_tasks.borrow().iter() {
            info!(
                "{} completed {:?}",
                game::get_object_by_id_typed(completed_task).unwrap().name(),
                self.tasks.get(completed_task).unwrap()
            );
            self.tasks.remove(completed_task);
        }
        for cancelled_task in cancelled_tasks.borrow().iter() {
            info!(
                "{} did not successfully complete {:?}",
                game::get_object_by_id_typed(cancelled_task).unwrap().name(),
                self.tasks.get(cancelled_task).unwrap()
            );
            self.tasks.remove(cancelled_task);
        }
        for (creep_id, task) in switch_tasks.borrow_mut().drain() {
            info!("{}'s task was switched to {:?}", creep_id, task);
            self.tasks.insert(creep_id, task);
        }
    }

    pub fn assign_tasks(&mut self) -> Vec<Box<dyn Task>> {
        self.recalculate_working_creeps();
        let idle_creeps = self.get_idle_creeps();
        let mut flag_tasks = self.get_flag_tasks();
        let mut room_tasks_map = HashMap::new();
        for room in game::rooms().values() {
            room_tasks_map.insert(room.name(), self.get_room_tasks(room));
        }

        for creep in idle_creeps {
            let current_room = creep.room();
            if current_room.is_none() {
                continue;
            }
            let current_room = current_room.unwrap();

            if let Some(task) = get_task_for_creep(&creep, &mut flag_tasks) {
                self.add_task(&creep, task);
                continue;
            }

            if let Some(room_tasks) = room_tasks_map.get_mut(&current_room.name()) {
                if let Some(task) = get_task_for_creep(&creep, room_tasks) {
                    self.add_task(&creep, task);
                    continue;
                }
            }

            let creep_type = get_creep_type(&creep);
            for (room_name, room_tasks) in room_tasks_map.iter_mut() {
                if room_name == &current_room.name() {
                    continue;
                }

                // Only send creeps to another room if they're not already working in that room
                // if let Some(working_creeps) = self.working_creeps_by_room.get(room_name) {
                //     if let Some(creep_count) = working_creeps.get(&creep_type) {
                //         if *creep_count > 3 {
                //             continue;
                //         }
                //     }
                // }

                if let Some(task) = get_task_for_creep(&creep, room_tasks) {
                    self.add_task(&creep, task);
                    continue;
                }
            }

            if let Some(task) = get_default_task_for_creep(&creep) {
                self.add_task(&creep, task)
            }
        }

        flag_tasks
    }

    fn get_flag_tasks(&self) -> Vec<Box<dyn Task>> {
        let mut tasks: Vec<Box<dyn Task>> = Vec::new();
        let flags = game::flags().values();

        for flag in flags {
            if flag.name().starts_with("claim", 0) {
                let room_name: String = flag
                    .name()
                    .split(":")
                    .pop()
                    .as_string()
                    .unwrap_or("".to_string());

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

    fn get_room_tasks(&self, room: Room) -> Vec<Box<dyn Task>> {
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
            }
        }

        // towers
        let towers = structures
            .iter()
            .filter(|s| s.structure_type() == StructureType::Tower);
        for tower in towers {
            if let StructureObject::StructureTower(tower) = tower {
                if tower.is_active()
                    && tower.store().get_free_capacity(Some(ResourceType::Energy)) > 0
                {
                    if let Some(id) = tower.try_id() {
                        tasks.push(Box::new(TransferTask::new(id)));
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

        // spawn
        let spawns = structures
            .iter()
            .filter(|s| s.structure_type() == StructureType::Spawn);

        for spawn in spawns {
            if let StructureObject::StructureSpawn(spawn) = spawn {
                if spawn.is_active()
                    && spawn.store().get_free_capacity(Some(ResourceType::Energy)) > 0
                {
                    if let Some(id) = spawn.try_id() {
                        tasks.push(Box::new(TransferTask::new(id)));
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
                    if road.hits() > road.hits_max() / 2 {
                        continue;
                    }
                } else if let StructureObject::StructureRampart(road) = structure {
                    if road.hits() > 100000 {
                        continue;
                    }
                }
                let id = s.try_id().unwrap();
                tasks.push(Box::new(RepairTask::new(id)));
            }
        }

        tasks
    }

    fn get_idle_creeps(&self) -> Vec<Creep> {
        let creeps = game::creeps().values();
        let mut idle_creeps: Vec<Creep> = Vec::new();

        for creep in creeps {
            // Creep doesn't exist
            let id = creep.try_id();
            if id.is_none() {
                continue;
            }
            let id = id.unwrap();

            // Creep already has a task assigned
            if let Some(_task) = self.tasks.get(&id) {
                continue;
            }

            // Creep is idle
            idle_creeps.push(creep.clone());
        }

        idle_creeps
    }
}

/// Returns the most appropriate task for the creep based on its body parts (if one exists)
fn get_task_for_creep(creep: &Creep, task_list: &mut Vec<Box<dyn Task>>) -> Option<Box<dyn Task>> {
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
                    } else {
                        // There are no sources to gather from and the creep has no energy
                        // so do nothing
                        return None;
                    }
                } else {
                    // Go back to an owned room if we can't harvest in the current room
                    return get_travel_home_task(creep);
                }
            }
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
            return Some(Box::new(TravelTask::new(controller.id())));
        }
    } else if creep_parts.contains(&Part::Claim) {
        return None;
    } else if creep_parts.contains(&Part::Work)
        && creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0
    {
        return Some(Box::new(UpgradeTask::new(
            creep.room().unwrap().controller().unwrap().id(),
        )));
    }

    if !utils::is_mine(&creep.room().unwrap()) {
        return get_travel_home_task(creep);
    }

    None
}

fn get_travel_home_task(creep: &Creep) -> Option<Box<dyn Task>> {
    let rooms = screeps::game::rooms().values();
    let mut my_owned_rooms = rooms
        .filter(|room| {
            room.controller()
                .map(|c| {
                    c.owner()
                        .is_some_and(|o| o.username() == creep.owner().username())
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    // Sort rooms by distance to creep (closest first)
    // TODO: this is probably not the best way to do this but it works for now
    my_owned_rooms.sort_by(|a, b| {
        creep
            .pos()
            .get_range_to(a.controller().unwrap().pos())
            .cmp(&creep.pos().get_range_to(b.controller().unwrap().pos()))
    });

    if let Some(room) = my_owned_rooms.first() {
        Some(Box::new(TravelTask::new(room.controller().unwrap().id())))
    } else {
        None
    }
}

type CompleteCallback = Box<dyn FnOnce(ObjectId<Creep>)>;
type CancelCallback = Box<dyn FnOnce(ObjectId<Creep>)>;
type SwitchCallback = Box<dyn FnOnce(ObjectId<Creep>, Box<dyn Task>)>;

pub trait Task: Debug {
    fn execute(
        &self,
        creep: &Creep,
        complete: CompleteCallback,
        cancel: CancelCallback,
        switch: SwitchCallback,
    );

    fn get_target_pos(&self) -> Option<screeps::Position> {
        None
    }

    fn get_type(&self) -> TaskType;

    fn requires_body_parts(&self) -> Vec<screeps::Part> {
        vec![Part::Work, Part::Carry]
    }
}

#[derive(Debug, PartialEq)]
pub enum TaskType {
    Build,
    Harvest,
    Heal,
    Repair,
    Transfer,
    Upgrade,
    Attack,
    Move,
    Claim,
}
