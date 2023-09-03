use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, HasPosition, MaybeHasTypedId, ObjectId, Resolvable, ResourceType, SharedCreepProperties,
    Transferable,
};

pub struct TransferTask<T: Transferable + Resolvable> {
    target: ObjectId<T>,
}

impl<T: Transferable + Resolvable> TransferTask<T> {
    pub fn new(target: ObjectId<T>) -> TransferTask<T> {
        TransferTask { target }
    }
}

impl<T: Transferable + Resolvable> super::Task for TransferTask<T> {
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

        if let Some(target) = self.target.resolve() {
            if creep.pos().is_near_to(target.pos()) {
                creep
                    .transfer(&target, ResourceType::Energy, None)
                    .unwrap_or_else(|e| {
                        warn!("couldn't transfer: {:?}", e);
                        cancel(creep.try_id().unwrap());
                    });
            } else {
                let _ = creep.move_to(&target);
            }
        } else {
            cancel(creep.try_id().unwrap());
        }
    }
}

impl<T: Transferable + Resolvable> Debug for TransferTask<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(structure) = self.target.resolve() {
            write!(
                f,
                "Transfering energy to ({}, {})",
                structure.pos().x().u8(),
                structure.pos().y().u8(),
            )
        } else {
            write!(f, "Transfer ({:?})", self.target)
        }
    }
}
