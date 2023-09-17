use std::fmt::Debug;

use log::*;
use screeps::{Creep, HasPosition, MaybeHasTypedId, ObjectId, Part, SharedCreepProperties};

pub struct AttackTask {
    target: ObjectId<Creep>,
}

impl AttackTask {
    pub fn new(target: ObjectId<Creep>) -> AttackTask {
        AttackTask { target }
    }
}

impl super::Task for AttackTask {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Attack
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
    ) {
        let target_creep = self.target.resolve();
        if target_creep.is_none() {
            info!("cannot attack nonexistent creep");
            cancel(creep.try_id().unwrap());
            return;
        }

        let target_creep = target_creep.unwrap();

        if target_creep.hits() > 0 {
            if creep.pos().is_near_to(target_creep.pos()) {
                creep.attack(&target_creep).unwrap_or_else(|e| {
                    info!("failed to attack creep: {:?}", e);
                    cancel(creep.try_id().unwrap());
                });
            } else {
                let _ = creep.move_to(&target_creep);
            }
        } else {
            complete(creep.try_id().unwrap());
        }
    }

    fn requires_body_parts(&self) -> Vec<Part> {
        vec![Part::Attack]
    }

    fn get_target_pos(&self) -> Option<screeps::Position> {
        self.target.resolve().map(|target| target.pos())
    }

    fn requires_energy(&self) -> bool {
        false
    }

    fn get_icon(&self) -> String {
        String::from("⚔️")
    }
}

impl Debug for AttackTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(target_creep) = self.target.resolve() {
            write!(
                f,
                "Attack {} at ({}, {}) in room {} [{}/{}]",
                target_creep.name(),
                target_creep.pos().x().u8(),
                target_creep.pos().y().u8(),
                target_creep.pos().room_name(),
                target_creep.hits(),
                target_creep.hits_max()
            )
        } else {
            write!(f, "Attack ({:?})", self.target)
        }
    }
}
