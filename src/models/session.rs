use crate::utils::error::{Error, Result};
use crate::media::broadcaster::Broadcaster;
use crate::models::peer::PeerConnection;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Notify;

use webrtc::api::media_engine::MediaEngine;
use webrtc::api::media_engine::MIME_TYPE_VP8;
use webrtc::api::APIBuilder;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use webrtc::ice_transport::ice_candidate::{
    RTCIceCandidateInit,
    RTCIceCandidate,
};
use webrtc::ice_transport::ice_gatherer_state::RTCIceGathererState;


#[derive(Clone, Debug)]
pub struct Session {
    pub id: u64,
    pub uuid: String,
    pub peer_connections: Arc<Mutex<Vec<PeerConnection>>>,
    pub broadcaster: Broadcaster,
    pub queue: Arc<Mutex<Vec<String>>>,
} 

impl Session {

    pub async fn new() -> Result<Self> {

        let broadcaster = Broadcaster::new().await?;

        Ok(Self {
            id: 0,
            uuid: uuid::Uuid::new_v4().to_string(),
            peer_connections: Arc::new(Mutex::new(Vec::new())),
            broadcaster: broadcaster,
            queue: Arc::new(Mutex::new(Vec::new())),
        })
    }


    pub async fn create_peer(& mut self) -> Result<(String)> {

        println!("->> {:<12} - create_peer", "Session");

        let mut peer_connections = self.peer_connections.lock().await;
        let mut pc = PeerConnection::new(self.broadcaster.get_track().await.unwrap()).await;

        // uuid for identification
        let uuid = pc.uuid.clone();

        peer_connections.push(pc);
        Ok(uuid)
    }

    // add uuid for identification
    pub async fn get_offer(&self, peerid: String) -> Result<String> {

        println!("->> {:<12} - get_sdp_offer", "Session");

        let mut peer_connections = self.peer_connections.lock().await;
        match peer_connections.iter().find(|pc| pc.uuid == peerid) {
            Some(pc) => {
                let offer = pc.get_offer().await?;
                Ok(offer)
            },
            None => Err(Error::PeerConnectionNotFound { peerid }),
        }

    }

    pub async fn set_answer(&self, sdp: String, peerid: String) -> Result<()> {

        println!("->> {:<12} - set_sdp_answer", "Session");
        let mut peer_connections = self.peer_connections.lock().await;
        match peer_connections.iter().find(|pc| pc.uuid == peerid) {
            Some(pc) => {
                let offer = pc.set_answer(sdp).await?;
                Ok(())
            },
            None => Err(Error::PeerConnectionNotFound { peerid }),
        }
    }

    pub async fn get_ice(&self, peerid: String) -> Result<Vec<RTCIceCandidate>> {

        println!("->> {:<12} - get_ice", "Session");

        let mut peer_connections = self.peer_connections.lock().await;
        match peer_connections.iter().find(|pc| pc.uuid == peerid) {
            Some(pc) => {
                let offer = pc.get_ice().await?;
                Ok(offer)
            },
            None => Err(Error::PeerConnectionNotFound { peerid }),
        }
    }

    pub async fn add_ice(&self, candidate: RTCIceCandidateInit, peerid: String) -> Result<()> {

        println!("->> {:<12} - set_ice", "Session");

        let mut peer_connections = self.peer_connections.lock().await;
        match peer_connections.iter().find(|pc| pc.uuid == peerid) {
            Some(pc) => {
                let offer = pc.add_ice(candidate).await?;
                Ok(())
            },
            None => Err(Error::PeerConnectionNotFound { peerid }),
        }
    }
}


#[derive(Clone, Debug)]
pub struct SessionController{
    pub sessions: Arc<Mutex<Vec<Option<Session>>>>,
}


impl SessionController{

    pub async fn new() -> Result<Self> {
        Ok(Self {
            sessions: Arc::default(),
        })
    }

    pub async fn create_session(&self) -> Result<(Session)> {

        println!("->> {:<12} - create_session", "Controller");

        let mut sessions = self.sessions.lock().await;
        let id = sessions.len() as u64;
        let mut session = Session::new().await?;


        println!("session created");

        sessions.push(Some(session.clone()));

        Ok(session)
    }

    pub async fn get_session(&self, id: u64) -> Result<Session> {
        let sessions = self.sessions.lock().await;
        let session = sessions.get(id as usize).and_then(|f| f.clone());
        session.ok_or(Error::SessionNotFound { id })
    }
}

