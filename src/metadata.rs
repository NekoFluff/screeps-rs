use screeps::{
    find, look::LookResult, ConstructionSite, Creep, HasPosition, MaybeHasTypedId, Room, Source,
    StructureController, StructureLink, StructureObject, StructureSpawn, StructureStorage,
};

pub struct SourceInfo {
    pub non_wall_terrain_count: u32,
    pub nearby_creep_count: u32,
    pub nearby_source_harvester_count: u32,
    pub has_link: bool,
}

impl SourceInfo {
    pub fn new(source: &Source, ignore_creep: Option<&Creep>) -> SourceInfo {
        let non_wall_terrain_count = source
            .room()
            .unwrap()
            .look_at_area(
                source.pos().y().u8() - 1,
                source.pos().x().u8() - 1,
                source.pos().y().u8() + 1,
                source.pos().x().u8() + 1,
            )
            .iter()
            .filter(|o| {
                if let LookResult::Terrain(terrain) = o.look_result {
                    return terrain != screeps::Terrain::Wall;
                }
                false
            })
            .count() as u32;

        let nearby_creep_count = source
            .room()
            .unwrap()
            .look_at_area(
                source.pos().y().u8() - 1,
                source.pos().x().u8() - 1,
                source.pos().y().u8() + 1,
                source.pos().x().u8() + 1,
            )
            .iter()
            .filter(|o| {
                if let LookResult::Creep(creep) = &o.look_result {
                    if ignore_creep.is_some()
                        && creep.try_id().unwrap() == ignore_creep.unwrap().try_id().unwrap()
                    {
                        return false;
                    }
                    return true;
                }
                false
            })
            .count() as u32;

        let nearby_source_harvester_count = source
            .room()
            .unwrap()
            .look_at_area(
                source.pos().y().u8() - 1,
                source.pos().x().u8() - 1,
                source.pos().y().u8() + 1,
                source.pos().x().u8() + 1,
            )
            .iter()
            .filter(|o| {
                if let LookResult::Creep(creep) = &o.look_result {
                    if ignore_creep.is_some()
                        && creep.try_id().unwrap() == ignore_creep.unwrap().try_id().unwrap()
                    {
                        return false;
                    }

                    let creep_type = super::utils::get_creep_type(creep);
                    if creep_type == "source_harvester" {
                        return true;
                    }
                }
                false
            })
            .count() as u32;

        let has_link = source
            .room()
            .unwrap()
            .look_at_area(
                source.pos().y().u8() - 2,
                source.pos().x().u8() - 2,
                source.pos().y().u8() + 2,
                source.pos().x().u8() + 2,
            )
            .iter()
            .filter(|o| {
                if let LookResult::Structure(structure) = &o.look_result {
                    if structure.structure_type() == screeps::StructureType::Link {
                        return true;
                    }
                }
                false
            })
            .count()
            > 0;

        SourceInfo {
            non_wall_terrain_count,
            nearby_creep_count,
            nearby_source_harvester_count,
            has_link,
        }
    }
}

pub struct RoomInfo {
    pub room: Room,
    pub sources: Vec<SourceInfo>,
    pub structures: Vec<StructureObject>,
    pub my_structures: Vec<StructureObject>,
    pub my_spawns: Vec<StructureSpawn>,
    pub construction_sites: Vec<ConstructionSite>,
    pub controller: Option<StructureController>,
    pub links: LinkTypeMap,
}

impl RoomInfo {
    pub fn new(room: Room) -> RoomInfo {
        let sources = room
            .find(screeps::constants::find::SOURCES, None)
            .iter()
            .map(|source| SourceInfo::new(source, None))
            .collect();

        let structures = room.find(screeps::constants::find::STRUCTURES, None);

        let my_structures = room.find(screeps::constants::find::MY_STRUCTURES, None);

        let my_spawns = room.find(screeps::constants::find::MY_SPAWNS, None);

        let construction_sites = room.find(screeps::constants::find::CONSTRUCTION_SITES, None);

        let controller = room.controller();

        let links = LinkTypeMap::new(&room);

        RoomInfo {
            room,
            sources,
            structures,
            my_structures,
            my_spawns,
            construction_sites,
            controller,
            links,
        }
    }
}

#[derive(Default)]
pub struct LinkTypeMap {
    pub source_links: Vec<SourceLink>,
    pub storage_links: Vec<StorageLink>,
    pub controller_links: Vec<ControllerLink>,
    pub unknown_links: Vec<UnknownLink>,
}

pub struct SourceLink(pub StructureLink, pub Source);
pub struct StorageLink(pub StructureLink, pub StructureStorage);
pub struct ControllerLink(pub StructureLink, pub StructureController);

pub struct UnknownLink(StructureLink);

impl LinkTypeMap {
    pub fn new(room: &Room) -> Self {
        let mut map: LinkTypeMap = LinkTypeMap::default();

        let my_structures = room.find(find::MY_STRUCTURES, None);

        let links = my_structures.iter().filter_map(|s| {
            if let StructureObject::StructureLink(link) = s {
                return Some(link.clone());
            }
            None
        });

        let sources = room.find(find::SOURCES, None);

        let storages = my_structures
            .iter()
            .filter_map(|s| {
                if let StructureObject::StructureStorage(storage) = s {
                    return Some(storage.clone());
                }
                None
            })
            .collect::<Vec<StructureStorage>>();

        if let Some(controller) = room.controller() {
            'link_loop: for link in links {
                for source in sources.iter() {
                    if link.pos().in_range_to(source.pos(), 2) {
                        map.source_links
                            .push(SourceLink(link.clone(), source.clone()));
                        continue 'link_loop;
                    }
                }

                if link.pos().in_range_to(controller.pos(), 2) {
                    map.controller_links
                        .push(ControllerLink(link.clone(), controller.clone()));
                    continue;
                }

                for storage in storages.iter() {
                    if link.pos().in_range_to(storage.pos(), 2) {
                        map.storage_links
                            .push(StorageLink(link.clone(), storage.clone()));
                        continue 'link_loop;
                    }
                }

                map.unknown_links.push(UnknownLink(link.clone()));
            }
        }

        map
    }
}
