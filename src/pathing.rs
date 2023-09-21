use log::*;
use screeps::{
    pathfinder::{MultiRoomCostResult, SingleRoomCostResult},
    Creep, ErrorCode, FindPathOptions, HasPosition, Path,
};
use wasm_bindgen::JsValue;

pub trait MovesAlongCachedPath: Stuckable {
    fn get_cached_path(&self) -> Option<&Path>;
    fn set_cached_path(&mut self, path: Option<Path>);
    fn empty_cached_path(&mut self) {
        self.set_cached_path(None);
    }

    fn move_to<T: HasPosition>(&mut self, creep: &Creep, target: T) -> Result<(), ErrorCode> {
        if self.get_cached_path().is_none() || self.is_stuck() {
            self.recalculate_path(creep, target, !self.is_stuck());
            self.set_stuck_count(0);
        }

        self.move_along_cached_path(creep)
    }

    fn recalculate_path<T: HasPosition>(&mut self, creep: &Creep, target: T, ignore_creeps: bool) {
        if creep.room().unwrap().name() == target.pos().room_name() {
            let options: FindPathOptions<_, MultiRoomCostResult> =
                FindPathOptions::new().ignore_creeps(ignore_creeps);
            let path = creep.pos().find_path_to(&target.pos(), Some(options));
            self.set_cached_path(Some(path));
        } else {
            let options: FindPathOptions<_, SingleRoomCostResult> =
                FindPathOptions::new().ignore_creeps(ignore_creeps);
            let path = creep.pos().find_path_to(&target.pos(), Some(options));
            self.set_cached_path(Some(path));
        }
    }

    fn move_along_cached_path(&mut self, creep: &Creep) -> Result<(), ErrorCode> {
        if let Some(path) = self.get_cached_path() {
            let path_str = path.to_string();
            let result: Result<(), ErrorCode> = creep.move_by_path(&JsValue::from_str(&path_str));
            if result != Ok(()) {
                debug!("unable to move along cached path: {:?}", result);
                self.set_stuck_count(self.get_stuck_count() + 1);
            }

            self.set_stuck_count(0);
            return result;
        } else {
            debug!("no cached path to move along");
        }
        Ok(())
    }
}

pub trait Stuckable {
    fn is_stuck(&self) -> bool;
    fn get_stuck_count(&self) -> u32;
    fn set_stuck_count(&mut self, count: u32);
}
