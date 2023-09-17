use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, HasPosition, MaybeHasTypedId, ObjectId, OwnedStructureProperties, Part, RoomPosition,
    SharedCreepProperties,
};

pub struct ClaimTask {
    target: RoomPosition,
}

impl ClaimTask {
    pub fn new(target: RoomPosition) -> ClaimTask {
        ClaimTask { target }
    }
}

impl super::Task for ClaimTask {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Claim
    }

    fn requires_body_parts(&self) -> Vec<Part> {
        vec![Part::Claim]
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        _cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
    ) {
        let room_pos = &self.target;
        let current_room = creep.room().unwrap();

        if current_room.name() == room_pos.room_name() {
            let controller = current_room.controller().unwrap();
            if controller.my() {
                complete(creep.try_id().unwrap());
                return;
            }

            if creep.pos().is_near_to(controller.pos()) {
                creep.claim_controller(&controller).unwrap_or_else(|e| {
                    info!("couldn't claim controller: {:?}", e);
                });
            } else {
                creep.move_to(&controller).unwrap_or_else(|e| {
                    info!("couldn't move to controller: {:?}", e);
                });
            }
        } else {
            creep.move_to(room_pos.clone()).unwrap_or_else(|e| {
                info!("couldn't move to other room: {:?}", e);
            });
        }
    }

    fn get_target_pos(&self) -> Option<screeps::Position> {
        Some(self.target.pos())
    }

    fn requires_energy(&self) -> bool {
        false
    }
}

impl Debug for ClaimTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Claim controller at ({}, {}) in room {}",
            self.target.x(),
            self.target.y(),
            self.target.room_name()
        )
    }
}
