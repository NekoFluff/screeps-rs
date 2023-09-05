use screeps::{Creep, OwnedStructureProperties, Room, SharedCreepProperties};

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
