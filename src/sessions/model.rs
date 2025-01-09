// my in memory database layer

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// queue is a list of song ids, the queue can update dynamically
pub struct PlayQueue {
    pub queue: Vec<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Session {
    pub id: u64,
    pub queue: PlayQueue,
} 

#[derive(Clone)]
pub struct ModelController {
    pub sessions: Arc<Mutex<Vec<Session>>>,
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
    pub async fn create_session(&self, owner_id: u64) -> Result<(Session)> {
        let mut sessions = self.sessions.lock().unwrap();

        let id = store.len() as u64 + 1;
        let session = Session {
            id,
            queue: PlayQueue { queue: Vec::new() },
        };
        sessions.push(session.clone());

        Ok(session)
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.sessions.lock().unwrap();
        Ok(sessions.clone())
    }

    pub async fn delete_session(&self, id: u64) -> Result<(Session)> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .iter()
            .position(|s| s.id == id)
            .map(|i| sessions.remove(i))
            .ok_or(Error::SessionNotFound)?;

        sessions.ok_or(Error::SessionDeleteFailIdNotFound { id })
    }
}

