use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, HasPosition, HasStore, MaybeHasTypedId, ObjectId, Resolvable, ResourceType,
    SharedCreepProperties, Withdrawable,
};

pub struct WithdrawTask<T: Withdrawable + Resolvable + HasStore> {
    target: ObjectId<T>,
}

impl<T: Withdrawable + Resolvable + HasStore> WithdrawTask<T> {
    pub fn new(target: ObjectId<T>) -> WithdrawTask<T> {
        WithdrawTask { target }
    }
}

impl<T: Withdrawable + Resolvable + HasStore> super::Task for WithdrawTask<T> {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Withdraw
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
    ) {
        let target = self.target.resolve();
        if target.is_none() {
            error!("there is no target to withdraw energy from");
            cancel(creep.try_id().unwrap());
            return;
        }
        let target = target.unwrap();

        // If the creep has energy, or the target has no energy, complete the task
        if creep.store().get_used_capacity(Some(ResourceType::Energy))
            == creep.store().get_capacity(Some(ResourceType::Energy))
            || target.store().get_used_capacity(Some(ResourceType::Energy)) == 0
        {
            complete(creep.try_id().unwrap());
            return;
        }

        if creep.pos().is_near_to(target.pos()) {
            creep
                .withdraw(&target, ResourceType::Energy, None)
                .unwrap_or_else(|e| {
                    debug!("couldn't withdraw: {:?}", e);
                    cancel(creep.try_id().unwrap());
                });
        } else {
            let _ = creep.move_to(&target);
        }
    }

    fn get_target_pos(&self) -> Option<screeps::Position> {
        self.target.resolve().map(|target| target.pos())
    }

    fn requires_energy(&self) -> bool {
        false
    }

    fn get_icon(&self) -> String {
        String::from("⚡")
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
