use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

use log::*;
use screeps::{game, Creep, MaybeHasTypedId, ObjectId, SharedCreepProperties};

mod build;
mod harvest;
mod heal;
mod repair;
mod transfer;
mod upgrade;

pub use build::BuildTask;
pub use harvest::HarvestTask;
pub use heal::HealTask;
pub use repair::RepairTask;
pub use transfer::TransferTask;
pub use upgrade::UpgradeTask;

pub struct TaskManager {
    pub tasks: HashMap<ObjectId<Creep>, Box<dyn Task>>,
}

impl TaskManager {
    pub fn new() -> TaskManager {
        TaskManager {
            tasks: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, creep: &Creep, task: Box<dyn Task>) {
        info!("{} was assigned to {:?}", creep.name(), task);

        self.tasks.insert(creep.try_id().unwrap(), task);
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
}

pub trait Task: Debug {
    fn execute(
        &self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        switch: Box<dyn FnOnce(ObjectId<Creep>, Box<dyn super::Task>)>,
    );

    fn get_target_pos(&self) -> Option<screeps::Position> {
        None
    }

    fn get_type(&self) -> TaskType;
}

#[derive(Debug, PartialEq)]
pub enum TaskType {
    Build,
    Harvest,
    Heal,
    Repair,
    Transfer,
    Upgrade,
}
