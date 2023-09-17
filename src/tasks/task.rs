use std::fmt::Debug;

use screeps::{Creep, ObjectId, Part};

use super::TaskList;

type CompleteCallback = Box<dyn FnOnce(ObjectId<Creep>)>;
type CancelCallback = Box<dyn FnOnce(ObjectId<Creep>)>;
type SwitchCallback = Box<dyn FnOnce(ObjectId<Creep>, TaskList)>;

pub trait Task: Debug {
    fn execute(
        &mut self,
        creep: &Creep,
        complete: CompleteCallback,
        cancel: CancelCallback,
        switch: SwitchCallback,
    );

    /// Returns the position of the target of the task
    fn get_target_pos(&self) -> Option<screeps::Position> {
        None
    }

    /// Returns the priority of the task. Higher priority tasks will be executed first.
    /// 0 is the highest priority.
    fn get_priority(&self) -> u32 {
        0
    }

    /// Returns the type of the task
    fn get_type(&self) -> TaskType;

    /// Returns the body parts required to perform the task
    fn requires_body_parts(&self) -> Vec<screeps::Part> {
        vec![Part::Work, Part::Carry]
    }

    fn requires_energy(&self) -> bool {
        true
    }

    fn get_icon(&self) -> String {
        String::from("")
    }
}

#[derive(Debug, PartialEq)]
pub enum TaskType {
    Attack,
    Build,
    Claim,
    HarvestSource,
    Heal,
    Idle,
    IdleUntil,
    Repair,
    Transfer,
    Travel,
    TravelDumb,
    Upgrade,
    Withdraw,
}
