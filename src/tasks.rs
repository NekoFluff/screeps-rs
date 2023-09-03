use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

use log::*;
use screeps::{game, Creep, MaybeHasTypedId, ObjectId};

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
        self.tasks.insert(creep.try_id().unwrap(), task);
    }

    pub fn execute_tasks(&mut self) {
        let completed_tasks = Rc::new(RefCell::new(Vec::new()));
        let cancelled_tasks = Rc::new(RefCell::new(Vec::new()));
        for (creep_id, task) in self.tasks.iter() {
            if let Some(creep) = game::get_object_by_id_typed(creep_id) {
                let completed_tasks_clone = completed_tasks.clone();
                let cancelled_tasks_clone = cancelled_tasks.clone();
                task.execute(
                    &creep,
                    Box::new(move |creep_id| completed_tasks_clone.borrow_mut().push(creep_id)),
                    Box::new(move |creep_id| cancelled_tasks_clone.borrow_mut().push(creep_id)),
                );
            }
        }
        for completed_task in completed_tasks.borrow().iter() {
            info!(
                "task completed: {:?}",
                self.tasks.get(completed_task).unwrap()
            );
            self.tasks.remove(completed_task);
        }
        for cancelled_task in cancelled_tasks.borrow().iter() {
            info!(
                "task cancelled: {:?}",
                self.tasks.get(cancelled_task).unwrap()
            );
            self.tasks.remove(cancelled_task);
        }
    }
}

pub trait Task: Debug {
    fn execute(
        &self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
    );
}
