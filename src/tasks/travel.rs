use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, HasPosition, MaybeHasTypedId, ObjectId, Part, Resolvable, SharedCreepProperties,
};

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
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
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

        creep.move_to(target).unwrap_or_else(|e| match e {
            screeps::ErrorCode::Tired => {
                // ignore
            }
            _ => {
                info!("cant move to location: {:?}", e);
                cancel(creep.try_id().unwrap());
            }
        });
    }

    fn get_target_pos(&self) -> Option<screeps::Position> {
        self.target.resolve().map(|target| target.pos())
    }

    fn requires_body_parts(&self) -> Vec<screeps::Part> {
        vec![Part::Move]
    }

    fn requires_energy(&self) -> bool {
        false
    }
}

impl<T: HasPosition + Resolvable> Debug for TravelTask<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(target) = self.target.resolve() {
            write!(
                f,
                "Travel to ({}, {})in room {}",
                target.pos().x().u8(),
                target.pos().y().u8(),
                target.pos().room_name()
            )
        } else {
            write!(f, "Travel to unknown target")
        }
    }
}
