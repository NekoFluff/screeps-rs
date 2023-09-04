use std::fmt::Debug;

use log::*;
use screeps::{Creep, HasPosition, MaybeHasTypedId, ObjectId, ResourceType, SharedCreepProperties};

pub struct HealTask {
    target: ObjectId<Creep>,
}

impl HealTask {
    pub fn new(target: ObjectId<Creep>) -> HealTask {
        HealTask { target }
    }
}

impl super::Task for HealTask {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Heal
    }

    fn execute(
        &self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, Box<dyn super::Task>)>,
    ) {
        if creep.store().get_free_capacity(Some(ResourceType::Energy)) == 0 {
            complete(creep.try_id().unwrap());
            return;
        }

        if let Some(target_creep) = self.target.resolve() {
            if target_creep.hits() < target_creep.hits_max() {
                if creep.pos().is_near_to(target_creep.pos()) {
                    creep.heal(&target_creep).unwrap_or_else(|e| {
                        info!("couldn't heal: {:?}", e);
                        cancel(creep.try_id().unwrap());
                    });
                } else {
                    let _ = creep.move_to(&target_creep);
                }
            } else {
                complete(creep.try_id().unwrap());
            }
        } else {
            cancel(creep.try_id().unwrap());
        }
    }

    fn get_target_pos(&self) -> Option<screeps::Position> {
        self.target.resolve().map(|target| target.pos())
    }
}

impl Debug for HealTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(target_creep) = self.target.resolve() {
            write!(
                f,
                "Heal {} at ({}, {}) [{}/{}]",
                target_creep.name(),
                target_creep.pos().x().u8(),
                target_creep.pos().y().u8(),
                target_creep.hits(),
                target_creep.hits_max()
            )
        } else {
            write!(f, "Heal ({:?})", self.target)
        }
    }
}
