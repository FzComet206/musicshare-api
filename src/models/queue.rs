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
            return QueueAction::Next(self.next())
        }

        QueueAction::Pass
    }

    pub fn remove(&mut self, key: String) -> QueueAction {
        let index = self.queue.iter().position(|x| x[0] == key);
        match index {
            Some(i) => {
                if i < self.curr_index {
                    // shift current index left by 1, because everything got moved up
                    self.curr_index -= 1;
                }
                self.queue.remove(i);

                if i == self.curr_index {
                    // the removed track was the current track
                    if self.queue.is_empty() {
                        self.curr_index = 0;
                        return QueueAction::Stop;
                    } else {
                        // if the queue is not empty, “play the same index” which now holds the next track
                        return QueueAction::Next(self.next());
                    }
                }

                QueueAction::Pass
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

    pub fn get_id(&self) -> String {
        return self.curr_index.to_string();
    }
}