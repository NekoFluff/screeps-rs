use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, HasPosition, MaybeHasTypedId, ObjectId, Part, Position, SharedCreepProperties,
};

pub struct TravelDumbTask {
    target: Position,
}

impl TravelDumbTask {
    pub fn new(target: Position) -> TravelDumbTask {
        TravelDumbTask { target }
    }
}

impl super::Task for TravelDumbTask {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::TravelDumb
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, Box<dyn super::Task>)>,
    ) {
        if creep.pos().is_near_to(self.target) {
            complete(creep.try_id().unwrap());
            return;
        }

        creep.move_to(self.target).unwrap_or_else(|e| match e {
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
        Some(self.target)
    }

    fn requires_body_parts(&self) -> Vec<screeps::Part> {
        vec![Part::Move]
    }

    fn requires_energy(&self) -> bool {
        false
    }
}

impl Debug for TravelDumbTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Travel (dumb) to ({}, {})",
            self.target.x().u8(),
            self.target.y().u8()
        )
    }
}
