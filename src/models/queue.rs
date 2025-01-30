use std::collections::HashMap;
use crate::utils::error::{ Result, Error };

pub enum QueueAction {
    Next(String),
    Stop,
    Pass,
    NotFound,
}

#[derive(Clone, Debug)]
pub struct PlayQueue {
    queue: Vec<Vec<String>>,
    curr_index: usize,
}

impl PlayQueue {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            curr_index: 0,
        }
    }

    pub fn get_all(&self) -> Vec<Vec<String>> {
        self.queue.clone()
    }

    pub fn add(
        &mut self, 
        key: String, 
        title: String, 
    ) -> QueueAction {
        let item = vec![key.clone(), title];

        self.queue.push(item);
        // item added is the only item in the queue, return the key
        if self.queue.len() == 1 {
            return QueueAction::Next((key.clone()))
        }

        QueueAction::Pass
    }

    pub fn remove(&mut self, key: String) -> QueueAction {
        let index = self.queue.iter().position(|x| x[0] == key);
        match index {
            Some(i) => {
                if i == 0 {
                    if self.queue.len() > 1 {
                        self.queue.remove(i);
                        return QueueAction::Next(self.queue[0][0].clone());
                    } else {
                        self.queue.remove(i);
                        return QueueAction::Stop
                    }
                }  else {
                    self.queue.remove(i);
                }
                return QueueAction::Pass
            },
            None => QueueAction::NotFound,
        }
    }

    pub fn reorder(&mut self, key: String, new_index: usize) -> QueueAction {
        let index = self.queue.iter().position(|x| x[0] == key);
        match index {
            Some(i) => {
                let item = self.queue.remove(i);
                self.queue.insert(new_index, item);
                if new_index == 0 {
                    return QueueAction::Next(key);
                } 
                return QueueAction::Pass
            },
            None => QueueAction::NotFound,
        }
    }


    pub fn next(&mut self) -> String {

        if self.queue.len() == 0 {
            return String::from("");
        }

        if self.curr_index == self.queue.len() - 1 {
            self.curr_index = 0;
        } else {
            self.curr_index += 1;
        }

        while self.curr_index >= self.queue.len() && self.curr_index > 0 {
            self.curr_index -= 1;
        }
        self.queue[self.curr_index][0].clone()
    }
}