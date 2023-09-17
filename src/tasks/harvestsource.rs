use std::fmt::Debug;

use log::*;
use screeps::{
    Creep, ErrorCode, HasPosition, MaybeHasTypedId, ObjectId, ResourceType, SharedCreepProperties,
    Source, StructureObject,
};

use crate::utils;

pub struct HarvestSourceTask {
    target: ObjectId<Source>,
    move_failure_count: u32,
}

impl HarvestSourceTask {
    pub fn new(target: ObjectId<Source>) -> HarvestSourceTask {
        HarvestSourceTask {
            target,
            move_failure_count: 0,
        }
    }
}

impl super::Task for HarvestSourceTask {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::HarvestSource
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        switch: Box<dyn FnOnce(ObjectId<Creep>, Box<dyn super::Task>)>,
    ) {
        let creep_type = super::utils::get_creep_type(creep);
        let room = creep.room().unwrap();
        if creep.store().get_free_capacity(Some(ResourceType::Energy)) == 0 {
            let source_links = utils::get_source_links(&room);
            // transfer to closest source link
            if let Some(StructureObject::StructureLink(source_link)) = source_links
                .iter()
                .filter(|link| creep.pos().get_range_to(link.pos()) <= 2)
                .min_by_key(|link| creep.pos().get_range_to(link.pos()))
            {
                let mut next_task: Option<Box<dyn super::Task>> = None;
                if creep_type == "source_harvester" {
                    next_task = Some(Box::new(HarvestSourceTask::new(self.target)));
                }

                switch(
                    creep.try_id().unwrap(),
                    Box::new(super::transfer::TransferTask::new(
                        source_link.try_id().unwrap(),
                        next_task,
                    )),
                );
            } else {
                complete(creep.try_id().unwrap());
            }

            return;
        }

        if let Some(source) = self.target.resolve() {
            if creep.pos().is_near_to(source.pos()) {
                creep.harvest(&source).unwrap_or_else(|e| {
                    info!("couldn't harvest: {:?}", e);
                    cancel(creep.try_id().unwrap());
                });
            } else {
                let result = creep.move_to(&source);

                if result.is_err() && result.err().unwrap() != ErrorCode::Tired {
                    self.move_failure_count += 1;
                    if self.move_failure_count >= 3 {
                        cancel(creep.try_id().unwrap());
                    }
                } else {
                    self.move_failure_count = 0;
                }

                // creep.move_to(&source).unwrap_or_else(|_e| {
                //     // info!("couldn't move to harvest: {:?}", _e);

                //     let mut sources = room.find(find::SOURCES_ACTIVE, None);
                //     sources.sort_by_key(|a| 0 - a.energy());

                //     for new_source in sources {
                //         if source.try_id().unwrap() != new_source.try_id().unwrap() {
                //             switch(
                //                 creep.try_id().unwrap(),
                //                 Box::new(HarvestSourceTask::new(new_source.try_id().unwrap())),
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

    fn requires_energy(&self) -> bool {
        false
    }
}

impl Debug for HarvestSourceTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(source) = self.target.resolve() {
            write!(
                f,
                "Harvest at ({}, {}) in room {} [{}/{}]",
                source.pos().x().u8(),
                source.pos().y().u8(),
                source.pos().room_name(),
                source.energy(),
                source.energy_capacity()
            )
        } else {
            write!(f, "Harvest ({:?})", self.target)
        }
    }
}
