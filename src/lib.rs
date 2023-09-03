use std::cell::RefCell;
use std::collections::{hash_map::Entry, HashMap};

use log::*;
use screeps::{
    constants::{ErrorCode, Part, ResourceType},
    enums::StructureObject,
    find, game,
    local::ObjectId,
    objects::{Creep, Source, StructureController},
    prelude::*,
};
use screeps::{
    ConstructionSite, RoomObjectProperties, Structure, StructureExtension, StructureSpawn,
    StructureTower, StructureType,
};
use wasm_bindgen::prelude::*;

mod logging;

// add wasm_bindgen to any function you would like to expose for call from js
#[wasm_bindgen]
pub fn setup() {
    logging::setup_logging(logging::Info);
}

// this is one way to persist data between ticks within Rust's memory, as opposed to
// keeping state in memory on game objects - but will be lost on global resets!
thread_local! {
    static CREEP_TARGETS: RefCell<HashMap<String, CreepTarget>> = RefCell::new(HashMap::new());
}

// this enum will represent a creep's lock on a specific target object, storing a js reference
// to the object id so that we can grab a fresh reference to the object each successive tick,
// since screeps game objects become 'stale' and shouldn't be used beyond the tick they were fetched
#[derive(Clone)]
enum CreepTarget {
    Upgrade(ObjectId<StructureController>),
    Harvest(ObjectId<Source>),
    Build(ObjectId<ConstructionSite>),
    TransferToSpawn(ObjectId<StructureSpawn>),
    TransferToExtension(ObjectId<StructureExtension>),
    TransferToTower(ObjectId<StructureTower>),
    Heal(ObjectId<Creep>),
    Repair(ObjectId<Structure>),
}

// to use a reserved name as a function name, use `js_name`:
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    debug!("loop starting! CPU: {}", game::cpu::get_used());
    // mutably borrow the creep_targets refcell, which is holding our creep target locks
    // in the wasm heap
    CREEP_TARGETS.with(|creep_targets_refcell| {
        let mut creep_targets = creep_targets_refcell.borrow_mut();
        debug!("running creeps");
        for creep in game::creeps().values() {
            run_creep(&creep, &mut creep_targets);
        }
    });

    debug!("running spawns");
    let mut additional = 0;
    let creep_limit = 7;
    let creeps = game::creeps();
    for spawn in game::spawns().values() {
        if creeps.values().count() >= creep_limit {
            break;
        }
        info!(
            "running spawn {} [{}/{}]",
            String::from(spawn.name()),
            creeps.values().count(),
            creep_limit
        );

        let mut body = vec![Part::Move, Part::Move, Part::Carry, Part::Work];
        let base_cost = body.iter().map(|p| p.cost()).sum::<u32>();
        info!(
            "energy: {}/{}",
            spawn.room().unwrap().energy_available(),
            spawn.room().unwrap().energy_capacity_available()
        );

        info!("base body cost: {}", base_cost);

        let remaining_energy = spawn.room().unwrap().energy_capacity_available() - base_cost;
        let x = Part::Move.cost() + Part::Work.cost() + 1;
        let y = remaining_energy / x;
        for _ in 0..y {
            body.push(Part::Move);
            body.push(Part::Work);
        }

        info!(
            "new body cost: {}",
            body.iter().map(|p| p.cost()).sum::<u32>()
        );

        if spawn.room().unwrap().energy_available() >= body.iter().map(|p| p.cost()).sum() {
            // create a unique name, spawn.
            let name_base = game::time();
            let name = format!("{}-{}", name_base, additional);
            // note that this bot has a fatal flaw; spawning a creep
            // creates Memory.creeps[creep_name] which will build up forever;
            // these memory entries should be prevented (todo doc link on how) or cleaned up
            match spawn.spawn_creep(&body, &name) {
                Ok(()) => additional += 1,
                Err(e) => warn!("couldn't spawn: {:?}", e),
            }
        }
    }

    info!("Done! cpu: {}", game::cpu::get_used())
}

fn run_creep(creep: &Creep, creep_targets: &mut HashMap<String, CreepTarget>) {
    if creep.spawning() {
        return;
    }
    let structures: Vec<StructureObject> = creep.room().unwrap().find(find::STRUCTURES, None);
    let name = creep.name();
    debug!("running creep: {}", name);

    let creeps_upgrading = creep_targets
        .iter()
        .filter(|c| matches!(c.1, CreepTarget::Upgrade(_)))
        .count();

    let target = creep_targets.entry(name);
    match target {
        Entry::Occupied(entry) => {
            let creep_target = entry.get();
            match creep_target {
                CreepTarget::Upgrade(controller_id)
                    if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    if let Some(controller) = controller_id.resolve() {
                        creep
                            .upgrade_controller(&controller)
                            .unwrap_or_else(|e| match e {
                                ErrorCode::NotInRange => {
                                    let _ = creep.move_to(&controller);
                                }
                                _ => {
                                    warn!("couldn't upgrade: {:?}", e);
                                    entry.remove();
                                }
                            });
                    } else {
                        entry.remove();
                    }
                }
                CreepTarget::Build(construction_site_id)
                    if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    if let Some(construction_site) = construction_site_id.resolve() {
                        creep.build(&construction_site).unwrap_or_else(|e| match e {
                            ErrorCode::NotInRange => {
                                let _ = creep.move_to(&construction_site);
                            }
                            _ => {
                                warn!("couldn't build: {:?}", e);
                                entry.remove();
                            }
                        });
                    } else {
                        entry.remove();
                    }
                }
                CreepTarget::Harvest(source_id)
                    if creep.store().get_free_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    if let Some(source) = source_id.resolve() {
                        if creep.pos().is_near_to(source.pos()) {
                            creep.harvest(&source).unwrap_or_else(|e| {
                                warn!("couldn't harvest: {:?}", e);
                                entry.remove();
                            });
                        } else {
                            let _ = creep.move_to(&source);
                        }
                    } else {
                        entry.remove();
                    }
                }
                CreepTarget::TransferToSpawn(source_id)
                    if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    if let Some(source) = source_id.resolve() {
                        if creep.pos().is_near_to(source.pos()) {
                            creep
                                .transfer(&source, ResourceType::Energy, None)
                                .unwrap_or_else(|e| {
                                    warn!("couldn't transfer to spawn: {:?}", e);
                                    entry.remove();
                                });
                        } else {
                            let _ = creep.move_to(&source);
                        }
                    } else {
                        entry.remove();
                    }
                }
                CreepTarget::TransferToExtension(source_id)
                    if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    if let Some(source) = source_id.resolve() {
                        if creep.pos().is_near_to(source.pos()) {
                            creep
                                .transfer(&source, ResourceType::Energy, None)
                                .unwrap_or_else(|e| {
                                    warn!("couldn't transfer to extension: {:?}", e);
                                    entry.remove();
                                });
                        } else {
                            let _ = creep.move_to(&source);
                        }
                    } else {
                        entry.remove();
                    }
                }
                CreepTarget::TransferToTower(source_id)
                    if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    if let Some(source) = source_id.resolve() {
                        if creep.pos().is_near_to(source.pos()) {
                            creep
                                .transfer(&source, ResourceType::Energy, None)
                                .unwrap_or_else(|e| {
                                    warn!("couldn't transfer to tower: {:?}", e);
                                    entry.remove();
                                });
                        } else {
                            let _ = creep.move_to(&source);
                        }
                    } else {
                        entry.remove();
                    }
                }
                CreepTarget::Heal(creep_id) => {
                    if let Some(creep2) = creep_id.resolve() {
                        if creep2.hits() < creep2.hits_max() {
                            if creep.pos().is_near_to(creep2.pos()) {
                                creep.heal(&creep2).unwrap_or_else(|e| {
                                    warn!("couldn't heal: {:?}", e);
                                });
                                entry.remove();
                            } else {
                                let _ = creep.move_to(&creep2);
                            }
                        } else {
                            entry.remove();
                        }
                    } else {
                        entry.remove();
                    }
                }
                CreepTarget::Repair(structure_id)
                    if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    if let Some(structure) = structure_id.resolve() {
                        if creep.pos().is_near_to(structure.pos()) {
                            creep.repair(&structure).unwrap_or_else(|e| {
                                warn!("couldn't repair: {:?}", e);
                                entry.remove();
                            });
                        } else {
                            let _ = creep.move_to(&structure);
                        }
                    } else {
                        entry.remove();
                    }
                }
                _ => {
                    entry.remove();
                }
            };
        }
        Entry::Vacant(entry) => {
            // no target, let's find one depending on if we have energy
            let room = creep.room().expect("couldn't resolve creep room");
            if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 {
                // controller: if the downgrade time is less than 10000 ticks, upgrade
                for structure in structures.iter() {
                    if let StructureObject::StructureController(controller) = structure {
                        if controller.ticks_to_downgrade() < 10000 && controller.is_active() {
                            entry.insert(CreepTarget::Upgrade(controller.id()));
                            return;
                        }
                    }
                }

                // spawn
                if let Some(spawn) = room.find(find::MY_SPAWNS, None).get(0) {
                    if spawn.is_active()
                        && spawn.store().get_free_capacity(Some(ResourceType::Energy)) > 0
                    {
                        if let Some(id) = spawn.try_id() {
                            entry.insert(CreepTarget::TransferToSpawn(id));
                            return;
                        }
                    }
                }

                // towers
                let towers = structures
                    .iter()
                    .filter(|s| s.structure_type() == StructureType::Tower);
                for tower in towers {
                    if let StructureObject::StructureTower(tower) = tower {
                        if tower.is_active()
                            && tower.store().get_free_capacity(Some(ResourceType::Energy)) > 0
                        {
                            if let Some(id) = tower.try_id() {
                                entry.insert(CreepTarget::TransferToTower(id));
                                return;
                            }
                        }
                    }
                }

                // extensions
                let extensions = structures
                    .iter()
                    .filter(|s| s.structure_type() == StructureType::Extension);

                for extension in extensions {
                    if let StructureObject::StructureExtension(extension) = extension {
                        if extension.is_active()
                            && extension
                                .store()
                                .get_free_capacity(Some(ResourceType::Energy))
                                > 0
                        {
                            if let Some(id) = extension.try_id() {
                                entry.insert(CreepTarget::TransferToExtension(id));
                                return;
                            }
                        }
                    }
                }

                // healing
                if creep.hits() < creep.hits_max() {
                    entry.insert(CreepTarget::Heal(creep.try_id().unwrap()));
                    return;
                }

                // repair
                for structure in structures.iter() {
                    let s = structure.as_structure();
                    if s.hits() < s.hits_max() {
                        if let StructureObject::StructureWall(wall) = structure {
                            if wall.hits() > 25000 {
                                continue;
                            }
                        }
                        let id = s.try_id().unwrap();
                        entry.insert(CreepTarget::Repair(id));
                        return;
                    }
                }

                // controller: if the number of creeps upgrading is less than 2, upgrade
                if creeps_upgrading < 2 {
                    for structure in structures.iter() {
                        if let StructureObject::StructureController(controller) = structure {
                            entry.insert(CreepTarget::Upgrade(controller.id()));
                            return;
                        }
                    }
                }

                // general construction sites
                if let Some(construction_site) = creep
                    .pos()
                    .find_closest_by_path(find::CONSTRUCTION_SITES, None)
                {
                    if let Some(id) = construction_site.try_id() {
                        entry.insert(CreepTarget::Build(id));
                        return;
                    }
                }

                // controller
                for structure in room.find(find::STRUCTURES, None).iter() {
                    if let StructureObject::StructureController(controller) = structure {
                        entry.insert(CreepTarget::Upgrade(controller.id()));
                        return;
                    }
                }
            } else if let Some(source) = room.find(find::SOURCES_ACTIVE, None).get(0) {
                entry.insert(CreepTarget::Harvest(source.id()));
                return;
            }
        }
    }
}
