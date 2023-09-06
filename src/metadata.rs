use screeps::{look::LookResult, HasPosition, Source};

pub struct SourceInfo {
    pub non_wall_terrain_count: u32,
    pub nearby_creep_count: u32,
}

impl SourceInfo {
    pub fn new(source: &Source) -> SourceInfo {
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
                if let LookResult::Creep(_) = o.look_result {
                    return true;
                }
                false
            })
            .count() as u32;

        SourceInfo {
            non_wall_terrain_count,
            nearby_creep_count,
        }
    }
}
