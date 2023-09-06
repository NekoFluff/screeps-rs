use screeps::{
    Creep, HasPosition, OwnedStructureProperties, Room, SharedCreepProperties, StructureObject,
    StructureProperties, StructureType,
};

pub fn get_creep_type(creep: &Creep) -> String {
    creep
        .name()
        .chars()
        .take_while(|&ch| ch != '-')
        .collect::<String>()
}

pub fn is_mine(room: &Room) -> bool {
    room.controller()
        .map(|controller| controller.my())
        .unwrap_or(false)
}

pub fn get_source_links(room: &Room) -> Vec<StructureObject> {
    let my_structures = room.find(screeps::find::MY_STRUCTURES, None);

    my_structures
        .iter()
        .filter(|s| s.structure_type() == StructureType::Link)
        .filter(|s| {
            let sources = room.find(screeps::find::SOURCES, None);
            for source in sources.iter() {
                if s.pos().in_range_to(source.pos(), 2) {
                    return true;
                }
            }
            false
        })
        .cloned()
        .collect::<Vec<_>>()
}
