use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, HasPosition, MaybeHasTypedId, ObjectId, Part, Path, Position, SharedCreepProperties,
};

use crate::pathing::MovesAlongCachedPath;

pub struct TravelDumbTask {
    target: Position,
    cached_path: Option<Path>,
    stuck_count: u32,
}

impl TravelDumbTask {
    pub fn new(target: Position) -> TravelDumbTask {
        TravelDumbTask {
            target,
            cached_path: None,
            stuck_count: 0,
        }
    }
}

impl crate::pathing::MovesAlongCachedPath for TravelDumbTask {
    fn get_cached_path(&self) -> Option<&Path> {
        self.cached_path.as_ref()
    }

    fn set_cached_path(&mut self, path: Option<Path>) {
        self.cached_path = path;
    }
}

impl crate::pathing::Stuckable for TravelDumbTask {
    fn is_stuck(&self) -> bool {
        self.stuck_count > 5
    }

    fn get_stuck_count(&self) -> u32 {
        self.stuck_count
    }

    fn set_stuck_count(&mut self, count: u32) {
        self.stuck_count = count;
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
        _switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
    ) {
        if creep.pos().is_near_to(self.target) {
            complete(creep.try_id().unwrap());
            return;
        }

        self.move_to(creep, self.target)
            .unwrap_or_else(|e| match e {
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

    fn get_icon(&self) -> String {
        String::from("🚶")
    }
}

impl Debug for TravelDumbTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Travel (dumb) to ({}, {}) in room {}",
            self.target.x().u8(),
            self.target.y().u8(),
            self.target.room_name()
        )
    }
}
