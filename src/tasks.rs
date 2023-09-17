use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

use log::*;
use screeps::{
    find, game, Creep, HasHits, HasPosition, HasTypedId, MaybeHasTypedId, ObjectId,
    OwnedStructureProperties, Part, Position, ResourceType, Room, RoomName, RoomPosition,
    SharedCreepProperties, Source, StructureObject, StructureProperties, StructureType,
};

mod attack;
mod build;
mod claim;
mod harvestsource;
mod heal;
mod repair;
mod transfer;
mod travel;
mod travel_dumb;
mod upgrade;
mod withdraw;

pub use attack::AttackTask;
pub use build::BuildTask;
pub use claim::ClaimTask;
pub use harvestsource::HarvestSourceTask;
pub use heal::HealTask;
pub use repair::RepairTask;
pub use transfer::TransferTask;
pub use travel::TravelTask;
pub use travel_dumb::TravelDumbTask;
pub use upgrade::UpgradeTask;
pub use withdraw::WithdrawTask;

use crate::{
    utils::{self, get_creep_type},
    LinkTypeMap,
};
use wasm_bindgen::JsValue;

type TaskMap = HashMap<ObjectId<Creep>, TaskList>;

pub struct TaskManager {
    pub tasks: TaskMap,
    working_creeps_by_room_and_type: HashMap<RoomName, HashMap<String, u32>>,
    working_creeps_by_room_and_pos: HashMap<RoomName, HashMap<Position, u32>>,
    pub room_links: HashMap<RoomName, LinkTypeMap>,
}

impl TaskManager {
    pub fn new() -> TaskManager {
        let creeps = game::creeps();
        let mut working_creeps_by_room_and_type = HashMap::new();

        for creep in creeps.values() {
            let creep_type = get_creep_type(&creep);
            let room_name = creep.room().unwrap().name();

            let count: &mut HashMap<String, u32> = working_creeps_by_room_and_type
                .entry(room_name)
                .or_default();
            let creep_count = count.entry(creep_type).or_insert(0);
            *creep_count += 1;
        }

        TaskManager {
            tasks: HashMap::new(),
            working_creeps_by_room_and_type,
            working_creeps_by_room_and_pos: HashMap::new(),
            room_links: HashMap::new(),
        }
    }

    pub fn classify_links(&mut self) {
        self.room_links = HashMap::new();

        for room in game::rooms().values() {
            self.room_links
                .insert(room.name(), self.classify_links_for_room(&room));
        }
    }

    fn classify_links_for_room(&self, room: &Room) -> LinkTypeMap {
        let mut map: LinkTypeMap = LinkTypeMap::new();

        let my_structures = room.find(find::MY_STRUCTURES, None);

        let links = my_structures
            .iter()
            .filter(|s| s.structure_type() == StructureType::Link);

        let sources = room.find(find::SOURCES, None);

        let storages = my_structures
            .iter()
            .filter(|s| s.structure_type() == StructureType::Storage)
            .collect::<Vec<_>>();

        if let Some(controller) = room.controller() {
            'link_loop: for link in links {
                for source in sources.iter() {
                    if link.pos().in_range_to(source.pos(), 2) {
                        map.source_links.push(link.clone());
                        continue 'link_loop;
                    }
                }

                if link.pos().in_range_to(controller.pos(), 2) {
                    map.controller_links.push(link.clone());
                    continue;
                }

                for storage in storages.iter() {
                    if link.pos().in_range_to(storage.pos(), 2) {
                        map.storage_links.push(link.clone());
                        continue 'link_loop;
                    }
                }

                map.unknown_links.push(link.clone());
            }
        }

        map
    }

    fn execute_links(&self) {
        for link_map in self.room_links.values() {
            // info!(
            //     "links: source: {}, storage: {}, controller: {}, unknown: {}",
            //     link_map.source_links.len(),
            //     link_map.storage_links.len(),
            //     link_map.controller_links.len(),
            //     link_map.unknown_links.len()
            // );
            'source_loop: for link in link_map.source_links.iter() {
                if let StructureObject::StructureLink(source_link) = link {
                    if source_link
                        .store()
                        .get_used_capacity(Some(ResourceType::Energy))
                        > 0
                    {
                        for storage_link in link_map.storage_links.iter() {
                            if let StructureObject::StructureLink(storage_link) = storage_link {
                                if storage_link
                                    .store()
                                    .get_free_capacity(Some(ResourceType::Energy))
                                    > 50
                                {
                                    info!("transferring energy from source to storage");
                                    source_link
                                        .transfer_energy(storage_link, None)
                                        .unwrap_or_else(|e| {
                                            info!(
                                                "link couldn't transfer energy to storage: {:?}",
                                                e
                                            );
                                        });
                                    continue 'source_loop;
                                }
                            }
                        }

                        for controller_link in link_map.controller_links.iter() {
                            if let StructureObject::StructureLink(controller_link) = controller_link
                            {
                                if controller_link
                                    .store()
                                    .get_free_capacity(Some(ResourceType::Energy))
                                    > 50
                                {
                                    info!("transferring energy from source to controller");
                                    source_link
                                        .transfer_energy(controller_link, None)
                                        .unwrap_or_else(|e| {
                                            info!(
                                                "creep couldn't transfer energy to controller: {:?}",
                                                e
                                            );
                                        });
                                    continue 'source_loop;
                                }
                            }
                        }

                        // info!("link idle, no storage or controller links available");
                    }
                }
            }
        }
    }

    /// Removes tasks for creeps that no longer exist
    pub fn clean_up_tasks(&mut self) {
        let mut tasks_to_remove = Vec::new();
        for (creep_id, _task) in self.tasks.iter() {
            if game::get_object_by_id_typed(creep_id).is_none() {
                tasks_to_remove.push(*creep_id);
            }
        }

        for creep_id in tasks_to_remove {
            self.tasks.remove(&creep_id);
        }
    }

    fn recalculate_working_creeps_by_room_and_type(&mut self) {
        self.working_creeps_by_room_and_type = HashMap::new();

        for (creep_id, task_list) in self.tasks.iter_mut() {
            let creep = game::get_object_by_id_typed(creep_id);
            if creep.is_none() {
                continue;
            }

            let creep = creep.unwrap();
            let creep_type = get_creep_type(&creep);
            if creep_type == "attacker" || creep_type == "healer" {
                continue;
            }

            if let Some(task) = task_list.current_task() {
                let room_name = task
                    .get_target_pos()
                    .map(|p| p.room_name())
                    .unwrap_or(creep.room().unwrap().name());

                let count: &mut HashMap<String, u32> = self
                    .working_creeps_by_room_and_type
                    .entry(room_name)
                    .or_default();
                let creep_count = count.entry(creep_type).or_insert(0);
                *creep_count += 1;
            }
        }
    }

    fn recalculate_working_creeps_by_room_and_pos(&mut self) {
        self.working_creeps_by_room_and_pos = HashMap::new();

        for (creep_id, task_list) in self.tasks.iter_mut() {
            let creep = game::get_object_by_id_typed(creep_id);
            if creep.is_none() {
                continue;
            }

            let creep: Creep = creep.unwrap();
            let creep_type = get_creep_type(&creep);
            if creep_type == "attacker" || creep_type == "healer" {
                continue;
            }

            if let Some(task) = task_list.current_task() {
                let room_name = task
                    .get_target_pos()
                    .map(|p| p.room_name())
                    .unwrap_or(creep.room().unwrap().name());

                if let Some(target_pos) = task.get_target_pos() {
                    let count: &mut HashMap<Position, u32> = self
                        .working_creeps_by_room_and_pos
                        .entry(room_name)
                        .or_default();
                    let creep_count = count.entry(target_pos).or_insert(0);
                    *creep_count += 1;
                }
            }
        }
    }

    pub fn set_task_list(&mut self, creep: &Creep, task_list: TaskList) {
        if let Some(creep_id) = creep.try_id() {
            let task = task_list.current_task().unwrap();
            info!(
                "{} was assigned to {:?} at {:?}",
                creep.name(),
                task,
                task.get_target_pos()
            );
            let _ = js_sys::Reflect::set(
                &creep.memory(),
                &JsValue::from_str("task"),
                &JsValue::from_str(&format!("{:?}", task)),
            );

            if let Some(target_pos) = task.get_target_pos() {
                // Keep track of the position change
                if let Some(room) = self
                    .working_creeps_by_room_and_pos
                    .get_mut(&target_pos.room_name())
                {
                    *room.entry(target_pos).or_insert(0) += 1;
                }

                // Keep track of the room switch
                if target_pos.room_name() != creep.room().unwrap().name() {
                    // info!(
                    //     "{} switched rooms from {} to {}",
                    //     creep.name(),
                    //     creep.room().unwrap().name(),
                    //     target_pos.room_name()
                    // );

                    if let Some(room) = self
                        .working_creeps_by_room_and_type
                        .get_mut(&target_pos.room_name())
                    {
                        *room.entry(get_creep_type(creep)).or_insert(0) += 1;
                    }
                    if let Some(room) = self
                        .working_creeps_by_room_and_type
                        .get_mut(&creep.room().unwrap().name())
                    {
                        *room.entry(get_creep_type(creep)).or_insert(0) -= 1;
                    }
                }
            }

            self.tasks.insert(creep_id, task_list);
        }
    }

    pub fn execute_tasks(&mut self) {
        self.execute_links();

        let completed_tasks = Rc::new(RefCell::new(Vec::new()));
        let cancelled_tasks = Rc::new(RefCell::new(Vec::new()));
        let switch_tasks: Rc<RefCell<TaskMap>> = Rc::new(RefCell::new(HashMap::new()));

        for (creep_id, task_list) in self.tasks.iter_mut() {
            if let Some(creep) = game::get_object_by_id_typed(creep_id) {
                let completed_tasks_clone = completed_tasks.clone();
                let cancelled_tasks_clone = cancelled_tasks.clone();
                let switch_tasks_clone = switch_tasks.clone();
                if let Some(task) = task_list.current_task_mut() {
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
        }
        for completed_task in completed_tasks.borrow().iter() {
            info!(
                "{} completed {:?}",
                game::get_object_by_id_typed(completed_task).unwrap().name(),
                self.tasks
                    .get(completed_task)
                    .unwrap()
                    .current_task()
                    .unwrap(),
            );

            if self
                .tasks
                .get_mut(completed_task)
                .unwrap()
                .next_task()
                .is_none()
            {
                self.tasks.remove(completed_task);
            }
        }
        for cancelled_task in cancelled_tasks.borrow().iter() {
            info!(
                "{} did not successfully complete {:?}",
                game::get_object_by_id_typed(cancelled_task).unwrap().name(),
                self.tasks
                    .get(cancelled_task)
                    .unwrap()
                    .current_task()
                    .unwrap(),
            );
            if self
                .tasks
                .get_mut(cancelled_task)
                .unwrap()
                .next_task()
                .is_none()
            {
                self.tasks.remove(cancelled_task);
            }
        }
        for (creep_id, task_list) in switch_tasks.borrow_mut().drain() {
            let task = task_list.current_task().unwrap();
            info!(
                "{}'s task list was switched to {:?}",
                creep_id.resolve().unwrap().name(),
                task
            );
            if let Some(creep) = game::get_object_by_id_typed(&creep_id) {
                self.set_task_list(&creep, task_list);
            }
        }
    }

    pub fn assign_tasks(&mut self) -> Vec<TaskList> {
        let idle_creeps = self.get_idle_creeps();
        let mut flag_task_lists = self.get_flag_task_lists();
        let mut room_tasks_map = HashMap::new();
        for room in game::rooms().values() {
            room_tasks_map.insert(room.name(), self.get_room_task_lists(room));
        }

        'creep_loop: for creep in idle_creeps {
            let current_room = creep.room();
            if current_room.is_none() {
                continue;
            }
            let current_room = current_room.unwrap();

            if let Some(task) = self.get_task_list_for_creep(&creep, &mut flag_task_lists) {
                self.set_task_list(&creep, task);
                continue;
            }

            if let Some(room_tasks) = room_tasks_map.get_mut(&current_room.name()) {
                if let Some(task) = self.get_task_list_for_creep(&creep, room_tasks) {
                    self.set_task_list(&creep, task);
                    continue;
                }
            }

            for (room_name, room_tasks) in room_tasks_map.iter_mut() {
                if room_name == &current_room.name() {
                    continue;
                }

                // Only send creeps to another room if they're not already working in that room
                if let Some(working_creeps) = self.working_creeps_by_room_and_type.get(room_name) {
                    let creep_type = get_creep_type(&creep);
                    if let Some(creep_count) = working_creeps.get(&creep_type) {
                        if *creep_count > 0 {
                            // info!(
                            //     "{} has {} {} creeps working in it already",
                            //     room_name.to_string(),
                            //     creep_count,
                            //     creep_type
                            // );
                            continue;
                        }
                    }
                }

                if let Some(task) = self.get_task_list_for_creep(&creep, room_tasks) {
                    self.set_task_list(&creep, task);
                    continue 'creep_loop;
                }
            }

            if let Some(task) = self.get_default_task_list_for_creep(&creep) {
                self.set_task_list(&creep, task)
            }
        }

        self.recalculate_working_creeps_by_room_and_type();
        self.recalculate_working_creeps_by_room_and_pos();

        flag_task_lists
    }

    /// Returns the most appropriate task for the creep based on its body parts (if one exists)
    fn get_task_list_for_creep(
        &self,
        creep: &Creep,
        task_lists: &mut Vec<TaskList>,
    ) -> Option<TaskList> {
        // (index, task)
        let mut similar_tasks: Vec<(usize, &Box<dyn Task>)> = vec![];
        for (index, task_list) in task_lists.iter().enumerate() {
            let task = task_list.current_task().unwrap();
            if similar_tasks.is_empty() && can_creep_handle_task(creep, &**task) {
                if task.requires_energy()
                    && creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0
                    || !task.requires_energy()
                {
                    similar_tasks.push((index, task));
                    continue;
                }
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
            return Some(task_lists.remove(similar_tasks.get(0).unwrap().0));
        }

        // (index, distance to target)
        let mut tasks_by_value = similar_tasks
            .iter()
            .map(|t| {
                if t.1.get_type() == TaskType::Repair {
                    (t.0, t.1.get_priority())
                } else {
                    if let Some(target) = t.1.get_target_pos() {
                        let distance = creep.pos().get_range_to(target);
                        return (t.0, distance);
                    }
                    (t.0, u32::MAX)
                }
            })
            .collect::<Vec<(usize, u32)>>();

        tasks_by_value.sort_by(|a, b| a.1.cmp(&b.1));
        // info!("sorted tasks: {:?}", tasks_by_value);

        let shortest_distance_idx = tasks_by_value.first().unwrap().0;

        Some(task_lists.remove(shortest_distance_idx))
    }

    fn get_flag_task_lists(&self) -> Vec<TaskList> {
        let mut task_lists: Vec<TaskList> = Vec::new();
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
                    let task = Box::new(ClaimTask::new(room_pos));
                    task_lists.push(TaskList::new(vec![task], false));
                } else {
                    error!("invalid room name: {}", room_name);
                    flag.remove();
                }
            }
        }

        task_lists
    }

    fn get_room_task_lists(&self, room: Room) -> Vec<TaskList> {
        let controller = room.controller();
        if controller.is_none() {
            return Vec::new();
        }

        let controller = controller.unwrap();

        let mut tasks: Vec<TaskList> = Vec::new();

        let structures = room.find(find::STRUCTURES, None);
        let construction_sites = room.find(find::CONSTRUCTION_SITES, None);
        let enemy_creeps = room.find(find::HOSTILE_CREEPS, None);

        // attack
        if !enemy_creeps.is_empty() {
            for enemy_creep in enemy_creeps {
                if let Some(id) = enemy_creep.try_id() {
                    tasks.push(TaskList::new(vec![Box::new(AttackTask::new(id))], false));
                    tasks.push(TaskList::new(vec![Box::new(AttackTask::new(id))], false));
                }
            }
        }

        // controller: if the downgrade time is less than 10000 ticks, upgrade
        if controller.my() && controller.is_active() {
            if controller.ticks_to_downgrade() < 9000 {
                tasks.push(TaskList::new(
                    vec![Box::new(UpgradeTask::new(controller.id()))],
                    false,
                ));
            }

            if controller.level() < 2 {
                tasks.push(TaskList::new(
                    vec![Box::new(UpgradeTask::new(controller.id()))],
                    false,
                ));
            }
        }

        // towers
        let towers = structures
            .iter()
            .filter(|s| s.structure_type() == StructureType::Tower);
        for tower in towers {
            if let StructureObject::StructureTower(tower) = tower {
                if tower.is_active()
                    && tower.store().get_free_capacity(Some(ResourceType::Energy)) as u32
                        > tower.store().get_capacity(Some(ResourceType::Energy)) / 2
                {
                    if let Some(id) = tower.try_id() {
                        tasks.push(allow_withdrawal_from_storage(
                            &room,
                            Box::new(TransferTask::new(id)),
                        ));
                    }
                }
            }
        }

        // extensions
        let extensions = structures
            .iter()
            .filter(|s| s.structure_type() == StructureType::Extension);
        let mut extension_transfer_tasks_exist = false;
        for extension in extensions {
            if let StructureObject::StructureExtension(extension) = extension {
                if extension.is_active()
                    && extension
                        .store()
                        .get_free_capacity(Some(ResourceType::Energy))
                        > 0
                    && extension.my()
                {
                    if self.is_pos_being_worked_on(&room.name(), &extension.pos(), 1) {
                        continue;
                    }

                    if let Some(id) = extension.try_id() {
                        let transfer_task = Box::new(TransferTask::new(id));

                        tasks.push(allow_withdrawal_from_storage(&room, transfer_task));

                        extension_transfer_tasks_exist = true;
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
                        let transfer_task = Box::new(TransferTask::new(id));

                        tasks.push(allow_withdrawal_from_storage(&room, transfer_task));
                    }
                }
            }
        }

        // transfer energy from link to controller
        for controller_link in self
            .room_links
            .get(&room.name())
            .unwrap()
            .controller_links
            .iter()
        {
            if let StructureObject::StructureLink(controller_link) = controller_link {
                if self.is_pos_being_worked_on(&room.name(), &controller_link.pos(), 1) {
                    continue;
                }

                if extension_transfer_tasks_exist {
                    continue;
                }

                if controller_link
                    .store()
                    .get_used_capacity(Some(ResourceType::Energy))
                    * 3
                    > controller_link
                        .store()
                        .get_capacity(Some(ResourceType::Energy))
                        * 2
                    && controller_link.pos().in_range_to(controller.pos(), 2)
                {
                    if let Some(id) = controller_link.try_id() {
                        let upgrade_task = Box::new(UpgradeTask::new(controller.id()));
                        let withdraw_task = Box::new(WithdrawTask::new(id));
                        tasks.push(TaskList::new(vec![withdraw_task, upgrade_task], false));
                    }
                }
            }
        }

        // transfer energy from link to storage
        for storage_link in self
            .room_links
            .get(&room.name())
            .unwrap()
            .storage_links
            .iter()
        {
            if let StructureObject::StructureLink(storage_link) = storage_link {
                if self.is_pos_being_worked_on(&room.name(), &storage_link.pos(), 1) {
                    continue;
                }

                if storage_link
                    .store()
                    .get_used_capacity(Some(ResourceType::Energy))
                    > 0
                {
                    if let Some(id) = storage_link.try_id() {
                        // get storage closest to link
                        let storage = structures
                            .iter()
                            .filter(|s| {
                                s.structure_type() == StructureType::Storage
                                    && s.pos().in_range_to(storage_link.pos(), 2)
                                    && s.as_has_store()
                                        .unwrap()
                                        .store()
                                        .get_free_capacity(Some(ResourceType::Energy))
                                        as u32
                                        > s.as_has_store()
                                            .unwrap()
                                            .store()
                                            .get_capacity(Some(ResourceType::Energy))
                                            / 2
                            })
                            .min_by(|a, b| {
                                storage_link
                                    .pos()
                                    .get_range_to(a.pos())
                                    .cmp(&storage_link.pos().get_range_to(b.pos()))
                            });

                        if let Some(storage) = storage {
                            if let StructureObject::StructureStorage(storage) = storage {
                                let transfer_task = Box::new(TransferTask::new(storage.id()));
                                let withdraw_task = Box::new(WithdrawTask::new(id));
                                tasks
                                    .push(TaskList::new(vec![withdraw_task, transfer_task], false));
                            } else {
                                let withdraw_task = Box::new(WithdrawTask::new(id));
                                tasks.push(TaskList::new(vec![withdraw_task], false));
                            }
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
                tasks.push(allow_withdrawal_from_storage(
                    &room,
                    Box::new(BuildTask::new(id)),
                ));
            }
        }

        // repair
        let mut repair_task_count = 0;
        let repair_task_limit = 3;
        for structure in structures.iter() {
            let s = structure.as_structure();
            if self.is_pos_being_worked_on(&room.name(), &s.pos(), 1) {
                continue;
            }

            if s.hits() < s.hits_max() / 2 {
                if let StructureObject::StructureWall(s) = structure {
                    if controller.level() < 3 {
                        continue;
                    }

                    if s.hits() > 25000 {
                        continue;
                    }
                } else if let StructureObject::StructureRoad(s) = structure {
                    if s.hits() > s.hits_max() / 2 {
                        continue;
                    }
                } else if let StructureObject::StructureRampart(s) = structure {
                    if s.hits() > 100000 {
                        continue;
                    }
                }

                let id = s.try_id().unwrap();
                tasks.push(allow_withdrawal_from_storage(
                    &room,
                    Box::new(RepairTask::new(id)),
                ));

                repair_task_count += 1;

                if repair_task_count > repair_task_limit {
                    break;
                }
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

    fn is_pos_being_worked_on(
        &self,
        room_name: &RoomName,
        pos: &Position,
        target_count: u32,
    ) -> bool {
        if let Some(room) = self.working_creeps_by_room_and_pos.get(room_name) {
            if let Some(count) = room.get(pos) {
                return *count >= target_count;
            }
        }
        false
    }

    fn get_default_task_list_for_creep(&self, creep: &Creep) -> Option<TaskList> {
        let creep_type = get_creep_type(creep);
        let creep_parts = creep.body().iter().map(|p| p.part()).collect::<Vec<Part>>();

        if creep_type == "source_harvester" {
            return self.get_harvest_source_task_list(creep, false, true);
        } else if creep_type == "upgrader" {
            let structure = self
                .room_links
                .get(&creep.room().unwrap().name())
                .unwrap()
                .controller_links
                .get(0)
                .unwrap();

            if let StructureObject::StructureLink(link) = structure {
                if let Some(controller) = creep.room().unwrap().controller() {
                    let upgrade_task = Box::new(UpgradeTask::new(controller.id()));
                    let withdraw_task = Box::new(WithdrawTask::new(link.try_id().unwrap()));
                    return Some(TaskList::new(vec![withdraw_task, upgrade_task], true));
                }
            }

            return None;
        }

        if creep_parts.contains(&Part::Attack) {
            if let Some(defend_flag) = game::flags().values().find(|f| f.name() == "defend") {
                if !creep.pos().in_range_to(defend_flag.pos(), 3) {
                    let task = Box::new(TravelDumbTask::new(defend_flag.pos()));
                    return Some(TaskList::new(vec![task], false));
                } else {
                    return None;
                }
            }

            let controller = creep.room().unwrap().controller().unwrap();
            if !creep.pos().in_range_to(controller.pos(), 3) {
                let task = Box::new(TravelTask::new(controller.id()));
                return Some(TaskList::new(vec![task], false));
            }
        } else if creep_parts.contains(&Part::Claim) {
            return None;
        } else if creep_parts.contains(&Part::Work) {
            if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 {
                let controller = creep.room().unwrap().controller().unwrap();
                if controller.my() {
                    let task = Box::new(UpgradeTask::new(
                        creep.room().unwrap().controller().unwrap().id(),
                    ));
                    return Some(TaskList::new(vec![task], false));
                }
                return self.get_harvest_source_task_list(creep, true, false);
            }

            return self.get_harvest_source_task_list(creep, true, false);
        }

        if !utils::is_mine(&creep.room().unwrap()) {
            if let Some(task) = get_travel_home_task(creep) {
                return Some(TaskList::new(vec![task], false));
            }
        }

        None
    }

    fn get_harvest_source_task_list(
        &self,
        creep: &Creep,
        active_only: bool,
        link_required: bool,
    ) -> Option<TaskList> {
        // Gather energy
        let room = creep.room().unwrap();
        if let Some(controller) = room.controller() {
            if controller.my() {
                let mut sources: Vec<Source>;
                if active_only {
                    sources = room.find(find::SOURCES_ACTIVE, None);
                } else {
                    sources = room.find(find::SOURCES, None);
                };

                if link_required {
                    let links = self.room_links.get(&room.name()).unwrap();

                    sources = sources
                        .drain(..)
                        .filter(|s| {
                            for source_link in links.source_links.iter() {
                                if let StructureObject::StructureLink(source_link) = source_link {
                                    if source_link.pos().in_range_to(s.pos(), 4) {
                                        return true;
                                    }
                                }
                            }

                            false
                        })
                        .collect::<Vec<_>>();
                }

                sources.sort_by_key(|s| {
                    let source_info = super::metadata::SourceInfo::new(s, Some(creep));
                    let mut cost = 0;

                    if let Some(room_data) = self.working_creeps_by_room_and_pos.get(&room.name()) {
                        cost += *room_data.get(&s.pos()).unwrap_or(&0) * 10
                            + creep.pos().get_range_to(s.pos());
                    }

                    let source_busy =
                        source_info.nearby_creep_count >= source_info.non_wall_terrain_count;
                    if source_busy {
                        cost += 20;
                    }

                    let has_dedicated_source_harvester =
                        source_info.nearby_source_harvester_count > 0;
                    if has_dedicated_source_harvester {
                        cost += 1000;
                    }

                    // info!(
                    //     "Source Travel Cost: {}: [range {}] {}",
                    //     s.pos(),
                    //     creep.pos().get_range_to(s.pos()),
                    //     cost
                    // );

                    cost
                });

                if let Some(source) = sources.first() {
                    let creep_type = get_creep_type(creep);
                    let source_links = utils::get_source_links(&room);

                    // transfer to closest source link and repeat if the creep is a source harvester
                    if creep_type == "source_harvester" {
                        if let Some(StructureObject::StructureLink(source_link)) = source_links
                            .iter()
                            .filter(|link| creep.pos().get_range_to(link.pos()) <= 2)
                            .min_by_key(|link| creep.pos().get_range_to(link.pos()))
                        {
                            let harvest_task = Box::new(HarvestSourceTask::new(source.id()));
                            let transfer_task = Box::new(TransferTask::new(source_link.id()));
                            return Some(TaskList::new(vec![harvest_task, transfer_task], true));
                        }
                    }

                    let harvest_task = Box::new(HarvestSourceTask::new(source.id()));
                    return Some(TaskList::new(vec![harvest_task], false));

                    //
                } else {
                    // There are no sources to gather from and the creep has no energy
                    // so do nothing
                    return None;
                }
            } else {
                // Go back to an owned room if we can't harvest in the current room
                if let Some(task) = get_travel_home_task(creep) {
                    return Some(TaskList::new(vec![task], false));
                }
            }
        }

        None
    }
}

fn get_travel_home_task(creep: &Creep) -> Option<Box<dyn Task>> {
    let rooms = screeps::game::rooms().values();
    let mut my_owned_rooms = rooms
        .filter(|room| room.controller().map(|c| c.my()).unwrap_or(false))
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

fn allow_withdrawal_from_storage(room: &Room, next_task: Box<dyn Task>) -> TaskList {
    let mut tasks = vec![next_task];
    let structures = room.find(find::STRUCTURES, None);
    let storage = structures
        .iter()
        .filter(|s| {
            s.structure_type() == StructureType::Storage
                && s.as_has_store()
                    .unwrap()
                    .store()
                    .get_used_capacity(Some(ResourceType::Energy))
                    > 0
        })
        .last();

    if let Some(StructureObject::StructureStorage(storage)) = storage {
        let withdraw_task = Box::new(WithdrawTask::new(storage.id()));

        tasks.insert(0, withdraw_task)
    }

    TaskList::new(tasks, false)
}

fn can_creep_handle_task(creep: &Creep, task: &dyn Task) -> bool {
    let creep_type = get_creep_type(creep);
    let creep_parts = creep.body().iter().map(|p| p.part()).collect::<Vec<Part>>();
    let task_parts = task.requires_body_parts();

    for part in task_parts {
        if !creep_parts.contains(&part) {
            return false;
        }
    }

    if creep_type == "source_harvester" {
        return task.get_type() == TaskType::HarvestSource;
    } else if creep_type == "upgrader" {
        return task.get_type() == TaskType::Upgrade;
    }

    true
}

type CompleteCallback = Box<dyn FnOnce(ObjectId<Creep>)>;
type CancelCallback = Box<dyn FnOnce(ObjectId<Creep>)>;
type SwitchCallback = Box<dyn FnOnce(ObjectId<Creep>, TaskList)>;

pub trait Task: Debug {
    fn execute(
        &mut self,
        creep: &Creep,
        complete: CompleteCallback,
        cancel: CancelCallback,
        switch: SwitchCallback,
    );

    /// Returns the position of the target of the task
    fn get_target_pos(&self) -> Option<screeps::Position> {
        None
    }

    /// Returns the priority of the task. Higher priority tasks will be executed first.
    /// 0 is the highest priority.
    fn get_priority(&self) -> u32 {
        0
    }

    /// Returns the type of the task
    fn get_type(&self) -> TaskType;

    /// Returns the body parts required to perform the task
    fn requires_body_parts(&self) -> Vec<screeps::Part> {
        vec![Part::Work, Part::Carry]
    }

    fn requires_energy(&self) -> bool {
        true
    }
}

#[derive(Debug, PartialEq)]
pub enum TaskType {
    Build,
    HarvestSource,
    Heal,
    Repair,
    Transfer,
    Upgrade,
    Attack,
    Travel,
    Claim,
    TravelDumb,
    Withdraw,
}

pub struct TaskList {
    tasks: Vec<Box<dyn Task>>,
    repeat: bool,
    current_task_idx: usize,
}

impl TaskList {
    pub fn new(tasks: Vec<Box<dyn Task>>, repeat: bool) -> Self {
        Self {
            tasks,
            repeat,
            current_task_idx: 0,
        }
    }

    pub fn current_task(&self) -> Option<&Box<dyn Task>> {
        self.tasks.get(self.current_task_idx)
    }

    pub fn current_task_mut(&mut self) -> Option<&mut Box<dyn Task>> {
        self.tasks.get_mut(self.current_task_idx)
    }

    pub fn next_task(&mut self) -> Option<&Box<dyn Task>> {
        if self.current_task_idx + 1 >= self.tasks.len() {
            if self.repeat {
                self.current_task_idx = 0;
            } else {
                return None;
            }
        } else {
            self.current_task_idx += 1;
        }

        self.tasks.get(self.current_task_idx)
    }
}
