// my in memory database layer

use crate::utils::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// queue is a list of song ids, the queue can update dynamically
#[derive(Clone, Debug, Serialize)]
pub struct PlayQueue {
    pub queue: Vec<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Session {
    pub id: u64,
    pub queue: PlayQueue,
    pub peer: String
} 

#[derive(Clone)]
pub struct ModelController {
    pub sessions: Arc<Mutex<Vec<Option<Session>>>>,
}

// Constructor
impl ModelController {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            sessions: Arc::default(),
        })
    }
}

impl ModelController {
    pub async fn create_session(&self) -> Result<(Session)> {
        let mut sessions = self.sessions.lock().unwrap();

        let id = sessions.len() as u64;
        let session = Session {
            id,
            queue: PlayQueue { queue: Vec::new() },
            peer: String::from("Test peer string"),
        };
        sessions.push(Some(session.clone()));

        Ok(session)
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.sessions.lock().unwrap();
        let copies = sessions.iter().filter_map(|t| t.clone()).collect();
        Ok(copies)
    }

    pub async fn delete_session(&self, id: u64) -> Result<(Session)> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.get_mut(id as usize).and_then( |f| f.take());

        session.ok_or(Error::SessionDeleteFailIdNotFound { id })
    }
}

