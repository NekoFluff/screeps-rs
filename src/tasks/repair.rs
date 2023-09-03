use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, HasPosition, MaybeHasTypedId, ObjectId, ResourceType, SharedCreepProperties, Structure,
};

pub struct RepairTask {
    target: ObjectId<Structure>,
}

impl RepairTask {
    pub fn new(target: ObjectId<Structure>) -> RepairTask {
        RepairTask { target }
    }
}

impl super::Task for RepairTask {
    fn execute(
        &self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
    ) {
        if creep.store().get_used_capacity(Some(ResourceType::Energy)) == 0 {
            complete(creep.try_id().unwrap());
            return;
        }

        if let Some(structure) = self.target.resolve() {
            if creep.pos().is_near_to(structure.pos()) {
                creep.repair(&structure).unwrap_or_else(|e| {
                    warn!("couldn't repair: {:?}", e);
                });
                if structure.hits() >= structure.hits_max()
                    || creep.store().get_used_capacity(Some(ResourceType::Energy)) == 0
                {
                    cancel(creep.try_id().unwrap());
                }
            } else {
                let _ = creep.move_to(&structure);
            }
        } else {
            complete(creep.try_id().unwrap());
        }
    }
}

impl Debug for RepairTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(structure) = self.target.resolve() {
            write!(
                f,
                "Repair {:?} at ({}, {}) [{}/{}]",
                structure.structure_type(),
                structure.pos().x().u8(),
                structure.pos().y().u8(),
                structure.hits(),
                structure.hits_max()
            )
        } else {
            write!(f, "Repair ({:?})", self.target)
        }
    }
}
