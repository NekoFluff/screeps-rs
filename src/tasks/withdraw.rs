use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, ErrorCode, HasPosition, HasStore, MaybeHasTypedId, ObjectId, Resolvable, ResourceType,
    SharedCreepProperties, StructureController, Withdrawable,
};

pub struct WithdrawTask<T: Withdrawable + Resolvable + HasStore> {
    target: ObjectId<T>,
    upgrade_controller_id: Option<ObjectId<StructureController>>,
    next_task: Option<Box<dyn super::Task>>,
}

impl<T: Withdrawable + Resolvable + HasStore> WithdrawTask<T> {
    pub fn new(
        target: ObjectId<T>,
        upgrade_controller_id: Option<ObjectId<StructureController>>,
        next_task: Option<Box<dyn super::Task>>,
    ) -> WithdrawTask<T> {
        WithdrawTask {
            target,
            upgrade_controller_id,
            next_task,
        }
    }
}

impl<T: Withdrawable + Resolvable + HasStore> super::Task for WithdrawTask<T> {
    fn get_type(&self) -> super::TaskType {
        if self.next_task.is_some() {
            return self.next_task.as_ref().unwrap().get_type();
        }
        super::TaskType::Withdraw
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        switch: Box<dyn FnOnce(ObjectId<Creep>, Box<dyn super::Task>)>,
    ) {
        let target = self.target.resolve();
        if target.is_none() {
            error!("there is no target to withdraw energy from");
            cancel(creep.try_id().unwrap());
            return;
        }
        let target = target.unwrap();

        if self.upgrade_controller_id.is_some()
            && creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0
        {
            // Switch to next task if we have one
            if let Some(controller_id) = self.upgrade_controller_id {
                if let Some(controller) = controller_id.resolve() {
                    creep
                        .upgrade_controller(&controller)
                        .unwrap_or_else(|e| match e {
                            ErrorCode::NotInRange => {
                                let _ = creep.move_to(&controller);
                            }
                            _ => {
                                info!("couldn't upgrade: {:?}", e);
                                cancel(creep.try_id().unwrap());
                            }
                        });
                } else {
                    cancel(creep.try_id().unwrap());
                }
            }

            return;
        }
        // If we're full, or the target is empty, switch to next task or complete
        else if creep.store().get_used_capacity(Some(ResourceType::Energy))
            == creep.store().get_capacity(Some(ResourceType::Energy))
            || target.store().get_used_capacity(Some(ResourceType::Energy)) == 0
        {
            if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 {
                if self.next_task.is_some() {
                    switch(creep.try_id().unwrap(), self.next_task.take().unwrap());
                } else {
                    complete(creep.try_id().unwrap());
                }
            } else {
                error!("can't switch to next task. no energy in creep.");

                cancel(creep.try_id().unwrap());
            }

            return;
        }

        if creep.pos().is_near_to(target.pos()) {
            creep
                .withdraw(&target, ResourceType::Energy, None)
                .unwrap_or_else(|e| {
                    error!("couldn't withdraw: {:?}", e);
                    cancel(creep.try_id().unwrap());
                });
        } else {
            let _ = creep.move_to(&target);
        }
    }

    fn get_target_pos(&self) -> Option<screeps::Position> {
        if self.next_task.is_some() {
            return self.next_task.as_ref().unwrap().get_target_pos();
        }
        self.target.resolve().map(|target| target.pos())
    }

    fn requires_energy(&self) -> bool {
        false
    }
}

impl<T: Withdrawable + Resolvable + HasStore> Debug for WithdrawTask<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(target) = self.target.resolve() {
            write!(
                f,
                "Withdraw energy at ({}, {}) in room {} [{}/{}]",
                target.pos().x().u8(),
                target.pos().y().u8(),
                target.pos().room_name(),
                target.store().get_used_capacity(None),
                target.store().get_capacity(None)
            )
        } else {
            write!(f, "Withdraw ({:?})", self.target)
        }
    }
}
