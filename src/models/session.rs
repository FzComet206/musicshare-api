// my in memory database layer
use crate::utils::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::media::broadcaster::Broadcaster;

// queue is a list of song ids, the queue can update dynamically
#[derive(Clone, Debug)]
pub struct PlayQueue {
    pub queue: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: u64,
    pub uuid: String,
    pub queue: PlayQueue,
    pub broadcaster: Broadcaster,
} 

impl Session {
    pub async fn connect(&self) -> Result<String> {

        // get sdp offer from broadcaster
        println!("->> {:<12} - connect", "Session");
        let sdp_offer = self.broadcaster.get_sdp_offer().await.unwrap();

        Ok(sdp_offer)
    }
}

#[derive(Clone, Debug)]
pub struct SessionController{
    pub sessions: Arc<Mutex<Vec<Option<Session>>>>,
}

impl SessionController {
    pub async fn get_session(&self, id: u64) -> Result<Session> {
        let sessions = self.sessions.lock().await;
        let session = sessions.get(id as usize).and_then(|f| f.clone());
        session.ok_or(Error::SessionDeleteFailIdNotFound{ id })
    }
}

// Constructor
impl SessionController {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            sessions: Arc::default(),
        })
    }
}

impl SessionController {
}

impl SessionController{
    pub async fn create_session(&self) -> Result<(Session)> {

        println!("->> {:<12} - create_session", "Controller");

        let mut sessions = self.sessions.lock().await;

        let id = sessions.len() as u64;
        let session = Session {
            id,
            uuid: uuid::Uuid::new_v4().to_string(),
            queue: PlayQueue { queue: Vec::new() },
            broadcaster: Broadcaster::new().await.unwrap(),
        };

        sessions.push(Some(session.clone()));

        Ok(session)
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.sessions.lock().await;
        let copies = sessions.iter().filter_map(|t| t.clone()).collect();
        Ok(copies)
    }

    pub async fn delete_session(&self, id: u64) -> Result<(Session)> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions.get_mut(id as usize).and_then( |f| f.take());

        session.ok_or(Error::SessionDeleteFailIdNotFound { id })
    }
}

