use std::fmt::Debug;

use screeps::{Creep, MaybeHasTypedId, ObjectId};

pub struct IdleTask {
    duration: u32,
}

impl IdleTask {
    pub fn new(duration: u32) -> IdleTask {
        IdleTask { duration }
    }
}
impl super::Task for IdleTask {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Idle
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        _cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
    ) {
        if self.duration == 0 {
            complete(creep.try_id().unwrap());
        } else {
            self.duration -= 1;
        }
    }

    fn get_icon(&self) -> String {
        String::from("🕐")
    }
}

impl Debug for IdleTask {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Idling. {} ticks remaining", self.duration)
    }
}
