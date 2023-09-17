use std::fmt::Debug;

use log::*;
use screeps::{
    ConstructionSite, Creep, ErrorCode, HasPosition, MaybeHasTypedId, ObjectId, ResourceType,
    SharedCreepProperties,
};

pub struct BuildTask {
    target: ObjectId<ConstructionSite>,
}

impl BuildTask {
    pub fn new(target: ObjectId<ConstructionSite>) -> BuildTask {
        BuildTask { target }
    }
}

impl super::Task for BuildTask {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Build
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        _switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
    ) {
        if creep.store().get_used_capacity(Some(ResourceType::Energy)) == 0 {
            complete(creep.try_id().unwrap());
            return;
        }

        if let Some(construction_site) = self.target.resolve() {
            creep.build(&construction_site).unwrap_or_else(|e| match e {
                ErrorCode::NotInRange => {
                    let _ = creep.move_to(&construction_site);
                }
                _ => {
                    info!("couldn't build: {:?}", e);
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

    fn get_icon(&self) -> String {
        String::from("🚧")
    }
}

impl Debug for BuildTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(construction_site) = self.target.resolve() {
            write!(
                f,
                "Build {:?} at ({}, {}) [{}/{}]",
                construction_site.structure_type(),
                construction_site.pos().x().u8(),
                construction_site.pos().y().u8(),
                construction_site.progress(),
                construction_site.progress_total()
            )
        } else {
            write!(f, "Build ({:?})", self.target)
        }
    }
}
