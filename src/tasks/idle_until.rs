use std::fmt::Debug;

use screeps::{Creep, MaybeHasTypedId, ObjectId};

type UntilFn<T> = fn(&Creep, &T) -> bool;

pub struct IdleUntilTask<T> {
    until: UntilFn<T>,
    pass_through: T,
}

impl<T> IdleUntilTask<T> {
    pub fn new(until: UntilFn<T>, pass_through: T) -> IdleUntilTask<T> {
        IdleUntilTask {
            until,
            pass_through,
        }
    }
}
impl<T> super::Task for IdleUntilTask<T> {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::IdleUntil
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        _cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
    ) {
        if (self.until)(creep, &self.pass_through) {
            complete(creep.try_id().unwrap());
        }
    }

    fn get_icon(&self) -> String {
        String::from("🕐")
    }
}

impl<T> Debug for IdleUntilTask<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Idling until something happens.")
    }
}
