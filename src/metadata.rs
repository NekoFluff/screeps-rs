use screeps::{look::LookResult, Creep, HasPosition, MaybeHasTypedId, Source};

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

                    let creep_type = super::utils::get_creep_type(&creep);
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
