use super::Task;

pub struct TaskList {
    tasks: Vec<Box<dyn Task>>,
    repeat: bool,
    current_task_idx: usize,
}

impl TaskList {
    pub fn new(tasks: Vec<Box<dyn Task>>, repeat: bool) -> Self {
        Self {
            tasks,
            repeat,
            current_task_idx: 0,
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
}
