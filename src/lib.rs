use std::cell::RefCell;
use std::collections::{hash_map::Entry, HashMap};
use std::fmt::Debug;

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
    ConstructionSite, Room, RoomObjectProperties, Structure, StructureExtension, StructureSpawn,
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

impl Debug for CreepTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreepTarget::Upgrade(id) => write!(f, "Upgrade({:?})", id),
            CreepTarget::Harvest(id) => {
                if let Some(source) = id.resolve() {
                    write!(
                        f,
                        "Harvest at ({}, {}) [{}/{}]",
                        source.pos().x().u8(),
                        source.pos().y().u8(),
                        source.energy(),
                        source.energy_capacity()
                    )
                } else {
                    write!(f, "Harvest ({:?})", id)
                }
            }
            CreepTarget::Build(id) => {
                if let Some(construction_site) = id.resolve() {
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
                    write!(f, "Build ({:?})", id)
                }
            }
            CreepTarget::TransferToSpawn(id) => write!(f, "TransferToSpawn({:?})", id),
            CreepTarget::TransferToExtension(id) => write!(f, "TransferToExtension({:?})", id),
            CreepTarget::TransferToTower(id) => write!(f, "TransferToTower({:?})", id),
            CreepTarget::Heal(id) => write!(f, "Heal({:?})", id),
            CreepTarget::Repair(id) => {
                if let Some(structure) = id.resolve() {
                    write!(
                        f,
                        "Repair {:?} at ({}, {}) [{}/{}]",
                        structure.structure_type(),
                        structure.pos().x().u8(),
                        structure.pos().y().u8(),
                        structure.hits(),
                        structure.hits_max()
                    )
                } else {
                    write!(f, "Repair ({:?})", id)
                }
            }
        }
    }
}

// to use a reserved name as a function name, use `js_name`:
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    let target_creep_count = 7;

    debug!(
        "loop starting! CPU: {}. Peak Malloc: {}. Total Memory: {}",
        game::cpu::get_used(),
        game::cpu::get_heap_statistics().peak_malloced_memory(),
        game::cpu::get_heap_statistics().total_heap_size()
    );
    // mutably borrow the creep_targets refcell, which is holding our creep target locks
    // in the wasm heap
    CREEP_TARGETS.with(|creep_targets_refcell| {
        let mut creep_targets = creep_targets_refcell.borrow_mut();
        debug!("running creeps");

        let creeps = game::creeps().values();
        let idle_creeps = creeps
            .filter(|c| match creep_targets.entry(c.name()) {
                Entry::Occupied(_) => false,
                Entry::Vacant(_) => true,
            })
            .collect::<Vec<Creep>>();
        let idle_creep_count = idle_creeps.len();

        let mut jobs = Vec::new();
        if let Some(creep) = idle_creeps.get(0) {
            jobs =
                get_creep_target_jobs(&creep.room().unwrap(), idle_creep_count, target_creep_count);
        }

        for creep in idle_creeps {
            // Idle creeps should always harvest energy first
            if creep.store().get_used_capacity(Some(ResourceType::Energy)) == 0 {
                if let Some(source) = creep
                    .room()
                    .unwrap()
                    .find(find::SOURCES_ACTIVE, None)
                    .get(0)
                {
                    let job = CreepTarget::Harvest(source.id());
                    let _ = js_sys::Reflect::set(
                        &creep.memory(),
                        &JsValue::from_str("job"),
                        &JsValue::from_str(&format!("{:?}", job)),
                    );
                    creep_targets.insert(creep.name(), job);
                    return;
                }
            }

            // Once they have enough energy, they can pick up a job
            let job = jobs.pop();
            if let Some(job) = job {
                info!("assigning {} to {:?}", creep.name(), job);
                let _ = js_sys::Reflect::set(
                    &creep.memory(),
                    &JsValue::from_str("job"),
                    &JsValue::from_str(&format!("{:?}", job)),
                );
                creep_targets.insert(creep.name(), job);
            }
        }

        for creep in game::creeps().values() {
            run_creep(&creep, &mut creep_targets);
        }
    });

    spawn_creeps(target_creep_count);

    info!(
        "Done! cpu: {} Peak Malloc: {}. Total Memory: {}",
        game::cpu::get_used(),
        game::cpu::get_heap_statistics().peak_malloced_memory(),
        game::cpu::get_heap_statistics().total_heap_size()
    );
}

fn spawn_creeps(target_creep_count: usize) {
    debug!("running spawns");
    let mut additional = 0;

    let creeps = game::creeps();
    for spawn in game::spawns().values() {
        if creeps.values().count() >= target_creep_count {
            break;
        }
        info!(
            "running spawn {} [{}/{}]",
            String::from(spawn.name()),
            creeps.values().count(),
            target_creep_count
        );

        let mut body = vec![Part::Move, Part::Move, Part::Carry, Part::Work];
        let base_cost = body.iter().map(|p| p.cost()).sum::<u32>();
        info!(
            "energy: {}/{}",
            spawn.room().unwrap().energy_available(),
            spawn.room().unwrap().energy_capacity_available()
        );

        info!("base body cost: {}", base_cost);

        if (spawn.room().unwrap().energy_available() > base_cost) {
            let remaining_energy =
                std::cmp::max(spawn.room().unwrap().energy_available() - base_cost, 0);
            let x = Part::Move.cost() + Part::Work.cost() + 1;
            let y = remaining_energy / x;
            info!("adding {} move/work pairs", y);
            for _ in 0..y {
                body.push(Part::Move);
                body.push(Part::Work);
            }
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
}

fn get_creep_target_jobs(
    room: &Room,
    max_jobs: usize,
    target_creep_count: usize,
) -> Vec<CreepTarget> {
    let mut creep_targets: Vec<CreepTarget> = Vec::new();

    let structures = room.find(find::STRUCTURES, None);
    let construction_sites = room.find(find::CONSTRUCTION_SITES, None);
    let controller = room.controller();

    // controller: if the downgrade time is less than 10000 ticks, upgrade
    if let Some(c) = controller {
        if let Some(owner) = c.owner() {
            if owner.username() == "CrazyFluff" && c.ticks_to_downgrade() < 10000 && c.is_active() {
                creep_targets.push(CreepTarget::Upgrade(c.id()));
                if creep_targets.len() >= max_jobs {
                    return creep_targets;
                }
            }
        }
    }

    // spawn
    if let Some(spawn) = room.find(find::MY_SPAWNS, None).get(0) {
        if spawn.is_active() && spawn.store().get_free_capacity(Some(ResourceType::Energy)) > 0 {
            if let Some(id) = spawn.try_id() {
                creep_targets.push(CreepTarget::TransferToSpawn(id));

                if game::creeps().values().count() < target_creep_count {
                    for _ in 0..max_jobs {
                        creep_targets.push(CreepTarget::TransferToSpawn(id));
                    }
                }

                if creep_targets.len() >= max_jobs {
                    return creep_targets;
                }
            }
        }
    }

    // towers
    let towers = structures
        .iter()
        .filter(|s| s.structure_type() == StructureType::Tower);
    for tower in towers {
        if let StructureObject::StructureTower(tower) = tower {
            if tower.is_active() && tower.store().get_free_capacity(Some(ResourceType::Energy)) > 0
            {
                if let Some(id) = tower.try_id() {
                    creep_targets.push(CreepTarget::TransferToTower(id));
                    if creep_targets.len() >= max_jobs {
                        return creep_targets;
                    }
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
                    creep_targets.push(CreepTarget::TransferToExtension(id));

                    if game::creeps().values().count() < target_creep_count {
                        for _ in 0..max_jobs {
                            creep_targets.push(CreepTarget::TransferToExtension(id));
                        }
                    }

                    if creep_targets.len() >= max_jobs {
                        return creep_targets;
                    }
                }
            }
        }
    }

    // healing
    // if creep.hits() < creep.hits_max() {
    //     info!("{} needs healing", creep.name());
    //     entry.insert(CreepTarget::Heal(creep.try_id().unwrap()));
    //     return;
    // }

    // repair
    for structure in structures.iter() {
        let s = structure.as_structure();
        if s.hits() < s.hits_max() / 2 {
            if let StructureObject::StructureWall(wall) = structure {
                if wall.hits() > 25000 {
                    continue;
                }
            } else if let StructureObject::StructureRoad(road) = structure {
                if road.hits() > 100 {
                    continue;
                }
            } else if let StructureObject::StructureRampart(road) = structure {
                if road.hits() > 100000 {
                    continue;
                }
            }
            let id = s.try_id().unwrap();
            creep_targets.push(CreepTarget::Repair(id));
            if creep_targets.len() >= max_jobs {
                return creep_targets;
            }
        }
    }

    // construction sites
    for construction_site in construction_sites.iter() {
        if let Some(id) = construction_site.try_id() {
            creep_targets.push(CreepTarget::Build(id));
            if creep_targets.len() >= max_jobs {
                return creep_targets;
            }
        }
    }

    info!("creep targets: {:?}", creep_targets.len());

    creep_targets
}

fn run_creep(creep: &Creep, creep_targets: &mut HashMap<String, CreepTarget>) {
    if creep.spawning() {
        return;
    }

    let name = creep.name();
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
                            });
                            // info!(
                            //     "{} repairing {} ({:?})",
                            //     creep.name(),
                            //     structure.id(),
                            //     structure.structure_type()
                            // );
                            if (structure.hits() >= structure.hits_max()
                                || creep.store().get_used_capacity(Some(ResourceType::Energy)) == 0)
                            {
                                entry.remove();
                            }
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
                if let Some(controller) = room.controller() {
                    entry.insert(CreepTarget::Upgrade(controller.id()));
                }
            } else {
                error!("creep {} has no target and no energy", creep.name());
            }
        }
    }
}
