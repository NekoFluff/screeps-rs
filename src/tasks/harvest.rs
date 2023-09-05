use std::fmt::Debug;

use log::*;
use screeps::{
    find, Creep, HasPosition, MaybeHasTypedId, ObjectId, ResourceType, SharedCreepProperties,
    Source,
};

pub struct HarvestTask {
    target: ObjectId<Source>,
}

impl HarvestTask {
    pub fn new(target: ObjectId<Source>) -> HarvestTask {
        HarvestTask { target }
    }
}

impl super::Task for HarvestTask {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Harvest
    }

    fn execute(
        &self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        switch: Box<dyn FnOnce(ObjectId<Creep>, Box<dyn super::Task>)>,
    ) {
        let room = creep.room().unwrap();
        if creep.store().get_free_capacity(Some(ResourceType::Energy)) == 0 {
            complete(creep.try_id().unwrap());
            return;
        }

        if let Some(source) = self.target.resolve() {
            if creep.pos().is_near_to(source.pos()) {
                creep.harvest(&source).unwrap_or_else(|e| {
                    info!("couldn't harvest: {:?}", e);
                    cancel(creep.try_id().unwrap());
                });
            } else {
                let _ = creep.move_to(&source);

                // creep.move_to(&source).unwrap_or_else(|_e| {
                //     // info!("couldn't move to harvest: {:?}", _e);

                //     let mut sources = room.find(find::SOURCES_ACTIVE, None);
                //     sources.sort_by_key(|a| 0 - a.energy());

                //     for new_source in sources {
                //         if source.try_id().unwrap() != new_source.try_id().unwrap() {
                //             switch(
                //                 creep.try_id().unwrap(),
                //                 Box::new(HarvestTask::new(new_source.try_id().unwrap())),
                //             );
                //             return;
                //         }
                //     }
                // });
            }
        } else {
            cancel(creep.try_id().unwrap());
        }
    }

    fn get_target_pos(&self) -> Option<screeps::Position> {
        self.target.resolve().map(|target| target.pos())
    }
}

impl Debug for HarvestTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(source) = self.target.resolve() {
            write!(
                f,
                "Harvest at ({}, {}) [{}/{}]",
                source.pos().x().u8(),
                source.pos().y().u8(),
                source.energy(),
                source.energy_capacity()
            )
        } else {
            write!(f, "Harvest ({:?})", self.target)
        }
    }
}
