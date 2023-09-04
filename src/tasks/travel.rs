use std::fmt::Debug;

use log::*;
use screeps::{Creep, HasPosition, MaybeHasTypedId, ObjectId, Resolvable, SharedCreepProperties};

pub struct TravelTask<T: HasPosition + Resolvable> {
    target: ObjectId<T>,
}

impl<T: HasPosition + Resolvable> TravelTask<T> {
    pub fn new(target: ObjectId<T>) -> TravelTask<T> {
        TravelTask { target }
    }
}

impl<T: HasPosition + Resolvable> super::Task for TravelTask<T> {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Travel
    }

    fn execute(
        &self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, Box<dyn super::Task>)>,
    ) {
        let target = self.target.resolve();
        if target.is_none() {
            cancel(creep.try_id().unwrap());
            return;
        }

        let target = target.unwrap();
        if creep.pos().is_near_to(target.pos()) {
            complete(creep.try_id().unwrap());
            return;
        }

        creep.move_to(target).unwrap_or_else(|e| {
            debug!("cant move to location: {:?}", e);
            cancel(creep.try_id().unwrap());
        });
    }

    fn get_target_pos(&self) -> Option<screeps::Position> {
        self.target.resolve().map(|target| target.pos())
    }
}

impl<T: HasPosition + Resolvable> Debug for TravelTask<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(target) = self.target.resolve() {
            write!(
                f,
                "Travel to ({}, {})",
                target.pos().x().u8(),
                target.pos().y().u8()
            )
        } else {
            write!(f, "Travel to unknown target")
        }
    }
}
