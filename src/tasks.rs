use std::{hash::Hash, collections::HashMap, sync::Arc, rc::Rc, cell::RefCell};

use screeps::{Creep, ObjectId, HasPosition, SharedCreepProperties, game, MaybeHasTypedId};
use log::*;

pub struct TaskManager {
    pub tasks: HashMap<ObjectId<Creep>, Box<dyn Task>>,
}

impl TaskManager {
    fn new() -> TaskManager {
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
                task.execute(&creep,
                     Box::new(move |creep_id| completed_tasks_clone.borrow_mut().push(creep_id)),
                     Box::new(move |creep_id| cancelled_tasks_clone.borrow_mut().push(creep_id))
                    );
            }
        }
        for completed_task in completed_tasks.borrow().iter() {
            info!("task completed: {:?}", completed_task);
            self.tasks.remove(&completed_task);
        }
        for cancelled_task in cancelled_tasks.borrow().iter() {
            info!("task cancelled: {:?}", cancelled_task);
            self.tasks.remove(&cancelled_task);
        }
    }
}

trait Task {
    fn execute(&self, creep: &Creep, complete: Box<dyn FnOnce(ObjectId<Creep>)>, cancel: Box<dyn FnOnce(ObjectId<Creep>)>);
}

struct HealTask {
    target: ObjectId<Creep>,
}

impl HealTask {
    fn new(target: ObjectId<Creep>) -> HealTask {
        HealTask {
            target,
        }
    }
}

impl Task for HealTask {
    fn execute(&self, creep: &Creep, complete: Box<dyn FnOnce(ObjectId<Creep>)>, cancel: Box<dyn FnOnce(ObjectId<Creep>)>) {
        if let Some(target_creep) = self.target.resolve() {
            if target_creep.hits() < target_creep.hits_max() {
                if creep.pos().is_near_to(target_creep.pos()) {
                    creep.heal(&target_creep).unwrap_or_else(|e| {
                        warn!("couldn't heal: {:?}", e);
                        cancel(creep.try_id().unwrap());
                    });
                    complete(creep.try_id().unwrap());
                } else {
                    let _ = creep.move_to(&target_creep);
                }
            } else {
                cancel(creep.try_id().unwrap());
            }
        } else {
            cancel(creep.try_id().unwrap());
        }
    }
}