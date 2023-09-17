use std::fmt::Debug;

use log::*;
use screeps::{
    find, Creep, HasPosition, HasStore, MaybeHasTypedId, ObjectId, Resolvable, ResourceType,
    SharedCreepProperties, StructureExtension, StructureObject, StructureProperties, Transferable,
};

pub struct TransferTask<T: Transferable + Resolvable + HasStore> {
    target: ObjectId<T>,
}

impl<T: Transferable + Resolvable + HasStore> TransferTask<T> {
    pub fn new(target: ObjectId<T>) -> TransferTask<T> {
        TransferTask { target }
    }

    fn get_nearest_extension(&self, creep: &Creep) -> Option<ObjectId<StructureExtension>> {
        // Get extensions that require energy and sort by distance
        let structures = creep.room().unwrap().find(find::MY_STRUCTURES, None);
        let mut extensions = structures
            .iter()
            .filter(|s| {
                if s.structure_type() == screeps::StructureType::Extension {
                    if let StructureObject::StructureExtension(extension) = s {
                        if extension
                            .store()
                            .get_free_capacity(Some(ResourceType::Energy))
                            > 0
                        {
                            return true;
                        }
                    }
                }
                false
            })
            .map(|s| {
                let pos = s.pos();
                (s, creep.pos().get_range_to(pos))
            })
            .collect::<Vec<_>>();

        extensions.sort_by(|(_, a), (_, b)| a.cmp(b));

        let extension = extensions.first();
        if let Some(extension_data) = extension {
            if let StructureObject::StructureExtension(extension) = extension_data.0 {
                return Some(extension.try_id().unwrap());
            }
        }
        None
    }
}

impl<T: Transferable + Resolvable + HasStore> super::Task for TransferTask<T> {
    fn get_type(&self) -> super::TaskType {
        super::TaskType::Transfer
    }

    fn execute(
        &mut self,
        creep: &Creep,
        complete: Box<dyn FnOnce(ObjectId<Creep>)>,
        cancel: Box<dyn FnOnce(ObjectId<Creep>)>,
        switch: Box<dyn FnOnce(ObjectId<Creep>, super::TaskList)>,
    ) {
        if creep.store().get_used_capacity(Some(ResourceType::Energy)) == 0 {
            complete(creep.try_id().unwrap());
            return;
        }

        let target = self.target.resolve();
        if target.is_none() {
            cancel(creep.try_id().unwrap());
            return;
        }

        let target = target.unwrap();
        let creep_type = super::utils::get_creep_type(creep);
        if creep_type != "source_harvester"
            && target.store().get_free_capacity(Some(ResourceType::Energy)) == 0
        {
            if let Some(extension_id) = self.get_nearest_extension(creep) {
                switch(
                    creep.try_id().unwrap(),
                    super::TaskList::new(vec![Box::new(TransferTask::new(extension_id))], false),
                );
            } else {
                complete(creep.try_id().unwrap());
            }
            return;
        }

        if creep.pos().is_near_to(target.pos()) {
            creep
                .transfer(&target, ResourceType::Energy, None)
                .unwrap_or_else(|e| {
                    info!("couldn't transfer: {:?}", e);

                    if creep_type != "source_harvester" {
                        if let Some(extension_id) = self.get_nearest_extension(creep) {
                            switch(
                                creep.try_id().unwrap(),
                                super::TaskList::new(
                                    vec![Box::new(TransferTask::new(extension_id))],
                                    false,
                                ),
                            );
                            return;
                        }
                    }

                    cancel(creep.try_id().unwrap());
                });
        } else {
            let _ = creep.move_to(&target);
        }
    }

    fn get_target_pos(&self) -> Option<screeps::Position> {
        self.target.resolve().map(|target| target.pos())
    }

    fn get_icon(&self) -> String {
        String::from("🚚")
    }
}

impl<T: Transferable + Resolvable + HasStore> Debug for TransferTask<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(structure) = self.target.resolve() {
            write!(
                f,
                "Transfer energy to ({}, {}) in room {}",
                structure.pos().x().u8(),
                structure.pos().y().u8(),
                structure.pos().room_name(),
            )
        } else {
            write!(f, "Transfer ({:?})", self.target)
        }
    }
}
