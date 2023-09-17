use core::panic;

use screeps::{
    Creep, HasPosition, OwnedStructureProperties, Room, RoomName, SharedCreepProperties, Source,
    StructureObject, StructureProperties, StructureType,
};

use log::*;

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

pub fn get_source_links(room: &Room) -> Vec<(StructureObject, Source)> {
    let my_structures = room.find(screeps::find::MY_STRUCTURES, None);

    my_structures
        .iter()
        .filter(|s| s.structure_type() == StructureType::Link)
        .map(|s| {
            let sources = room.find(screeps::find::SOURCES, None);
            for source in sources.iter() {
                if s.pos().in_range_to(source.pos(), 2) {
                    return Some((s.clone(), source.clone()));
                }
            }
            None
        })
        .filter(|s| s.is_some())
        .map(|s| s.unwrap())
        .collect::<Vec<_>>()
}

pub fn get_room_name(room_name_str: &str) -> RoomName {
    RoomName::new(&room_name_str).unwrap()
}

pub fn pause_script() {
    super::PAUSE_SCRIPT.with(|p| {
        *p.borrow_mut() = true;
    });
    panic!("Paused script");
}

pub fn log_cpu_usage(str: &str) {
    let cpu = screeps::game::cpu::get_used();
    let cpu_used_since_last_call = cpu - super::LAST_CPU_USAGE.with(|l| *l.borrow());
    trace!(
        "[{:.4} CPU DIFF | TOTAL {:.2}]: {}",
        cpu_used_since_last_call,
        cpu,
        str
    );
    super::LAST_CPU_USAGE.with(|l| *l.borrow_mut() = cpu);
}
