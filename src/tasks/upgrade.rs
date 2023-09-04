use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, ErrorCode, HasPosition, MaybeHasTypedId, ObjectId, ResourceType, SharedCreepProperties,
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

    fn get_target_pos(&self) -> Option<screeps::Position> {
        self.target.resolve().map(|target| target.pos())
    }
}

impl Debug for UpgradeTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(controller) = self.target.resolve() {
            write!(
                f,
                "Upgrade controller at ({}, {}) [{}/{}]",
                controller.pos().x().u8(),
                controller.pos().y().u8(),
                controller.progress(),
                controller.progress_total()
            )
        } else {
            write!(f, "Upgrade at unknown location")
        }
    }
}
