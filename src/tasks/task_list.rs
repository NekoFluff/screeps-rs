use std::fmt::{Debug, Formatter};

use super::Task;

pub struct TaskList {
    tasks: Vec<Box<dyn Task>>,
    repeat: bool,
    current_task_idx: usize,
    primary_task_idx: usize,
}

impl Debug for TaskList {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskList")
            .field("tasks", &self.tasks)
            .field("repeat", &self.repeat)
            .field("current_task_idx", &self.current_task_idx)
            .field("primary_task_idx", &self.primary_task_idx)
            .finish()
    }
}

impl TaskList {
    pub fn new(tasks: Vec<Box<dyn Task>>, repeat: bool, primary_task_idx: usize) -> Self {
        Self {
            tasks,
            repeat,
            current_task_idx: 0,
            primary_task_idx,
        }
    }

    pub fn current_task(&self) -> Option<&dyn Task> {
        return Some(self.tasks.get(self.current_task_idx)?.as_ref());
    }

    pub fn current_task_mut(&mut self) -> Option<&mut Box<dyn Task>> {
        self.tasks.get_mut(self.current_task_idx)
    }

    pub fn next_task(&mut self) -> Option<&dyn Task> {
        if self.current_task_idx + 1 >= self.tasks.len() {
            if self.repeat {
                self.current_task_idx = 0;
            } else {
                return None;
            }
        } else {
            self.current_task_idx += 1;
        }

        return Some(self.tasks.get(self.current_task_idx)?.as_ref());
    }

    pub fn get_primary_task(&self) -> Option<&dyn Task> {
        return Some(self.tasks.get(self.primary_task_idx)?.as_ref());
    }
}
