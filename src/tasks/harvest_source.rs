use std::fmt::Debug;

use log::*;
use screeps::{
    BodyPart, Creep, ErrorCode, HasPosition, MaybeHasTypedId, ObjectId, ResourceType,
    SharedCreepProperties, Source,
};

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
        _switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
    ) {
        if let Some(ticks_to_live) = creep.ticks_to_live() {
            if ticks_to_live <= 3 {
                cancel(creep.try_id().unwrap());
                return;
            }
        }

        let free_capacity = creep.store().get_free_capacity(Some(ResourceType::Energy));
        if free_capacity == 0 {
            complete(creep.try_id().unwrap());
            return;
        }

        if 10 > free_capacity {
            complete(creep.try_id().unwrap());
            return;
        }

        if let Some(source) = self.target.resolve() {
            if creep.pos().is_near_to(source.pos()) {
                creep.harvest(&source).unwrap_or_else(|e| {
                    debug!("couldn't harvest: {:?}", e);
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

    fn get_icon(&self) -> String {
        String::from("⛏️⚡")
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
