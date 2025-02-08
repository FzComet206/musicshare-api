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

    pub fn remove_by_id(&mut self, index: usize) -> QueueAction {

        if self.queue.is_empty() {
            return QueueAction::Pass;
        }

        match index {
            i => {
                if i < self.curr_index {
                    // shift current index left by 1, because everything got moved up
                    self.curr_index -= 1;
                    self.queue.remove(i);
                    return QueueAction::Pass;
                }

                if i == self.curr_index {
                    self.queue.remove(i);
                    // the removed track was the current track
                    if self.queue.is_empty() {
                        self.curr_index = 0;
                        return QueueAction::Stop;
                    } else {
                        if self.curr_index == self.queue.len() {
                            self.curr_index -= 1;
                        }
                        // if the queue is not empty, “play the same index” which now holds the next track
                        println!("remove same index, curr_index: {}", self.curr_index);
                        return QueueAction::Next(self.queue[self.curr_index][0].clone());
                    }
                }

                self.queue.remove(i);

                QueueAction::Pass
            },
            _ => QueueAction::NotFound,
        }
    }

    pub fn remove_by_key(&mut self, key: String) -> QueueAction {

        // remove all items with the same key
        let mut indexes = Vec::new();
        for (i, item) in self.queue.iter().enumerate() {
            if item[0] == key {
                indexes.push(i);
            }
        }

        if indexes.contains(&self.curr_index) {
            while !indexes.is_empty() {
                let index = indexes.pop().unwrap();

                if index < self.curr_index {
                    // shift current index left by 1, because everything got moved up
                    self.curr_index -= 1;
                }

                self.queue.remove(index);
            }

            if self.queue.is_empty() {
                self.curr_index = 0;
                return QueueAction::Stop;
            }
            
            return QueueAction::Next(self.queue[self.curr_index][0].clone());

        } else {
            while !indexes.is_empty() {
                let index = indexes.pop().unwrap();

                if index < self.curr_index {
                    // shift current index left by 1, because everything got moved up
                    self.curr_index-= 1;
                }

                self.queue.remove(index);
            }
            return QueueAction::Pass;
        }
    }

    pub fn reorder(&mut self, old_index: usize, new_index: usize) -> QueueAction {
        if old_index >= self.queue.len() || new_index > self.queue.len() {
            return QueueAction::NotFound;
        }

        if self.queue.is_empty() {
            return QueueAction::Pass; 
        }

        let item = self.queue.remove(old_index);
        self.queue.insert(new_index, item);

        if old_index == self.curr_index {
            self.curr_index = new_index;
            return QueueAction::Pass;
        } else if new_index == self.curr_index {
            // problematic if this happenes and the old index is smaller than the current index
            if old_index > self.curr_index {
                self.curr_index += 1;
            } else {
                self.curr_index -= 1;
            }
            return QueueAction::Pass;
        }

        if old_index < self.curr_index && new_index > self.curr_index {
            self.curr_index -= 1;
        } else if old_index > self.curr_index && new_index <= self.curr_index {
            self.curr_index += 1;
        }

        if self.curr_index >= self.queue.len() {
            self.curr_index = self.queue.len().saturating_sub(1);
        }

        // back move to front with curr_index cause the current index to change


        QueueAction::Pass
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

    pub fn prev(&mut self) -> String {
        if self.queue.len() == 0 {
            return String::from("");
        }

        if self.curr_index == 0 {
            self.curr_index = self.queue.len() - 1;
        } else {
            self.curr_index -= 1;
        }

        while self.curr_index >= self.queue.len() && self.curr_index > 0 {
            self.curr_index -= 1;
        }

        self.queue[self.curr_index][0].clone()
    }

    pub fn has_key(&self, key: String) -> bool {
        self.queue.iter().any(|x| x[0] == key)
    }

    pub fn get_title(&self, index: usize) -> String {
        if index >= self.queue.len() {
            return "".to_string();
        }

        self.queue[index][1].clone()
    }
    
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}