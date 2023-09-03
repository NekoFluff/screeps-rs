use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, ErrorCode, MaybeHasTypedId, ObjectId, ResourceType, SharedCreepProperties,
    StructureController,
};

pub struct UpgradeTask {
    target: ObjectId<StructureController>,
}

impl UpgradeTask {
    pub fn new(target: ObjectId<StructureController>) -> UpgradeTask {
        UpgradeTask { target }
    }
}

impl super::Task for UpgradeTask {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Upgrade
    }

    fn execute(
        &self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, Box<dyn super::Task>)>,
    ) {
        if creep.store().get_used_capacity(Some(ResourceType::Energy)) == 0 {
            complete(creep.try_id().unwrap());
            return;
        }

        if let Some(controller) = self.target.resolve() {
            creep
                .upgrade_controller(&controller)
                .unwrap_or_else(|e| match e {
                    ErrorCode::NotInRange => {
                        let _ = creep.move_to(&controller);
                    }
                    _ => {
                        warn!("couldn't upgrade: {:?}", e);
                        cancel(creep.try_id().unwrap());
                    }
                });
        } else {
            cancel(creep.try_id().unwrap());
        }
    }
}

impl Debug for UpgradeTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // f.debug_struct("UpgradeTask")
        //     .field("target", &self.target)
        //     .finish()
        write!(f, "Upgrade({:?})", self.target)
    }
}
