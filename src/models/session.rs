use crate::utils::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::media::broadcaster::Broadcaster;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::media_engine::MIME_TYPE_VP8;
use webrtc::api::APIBuilder;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;

#[derive(Clone, Debug)]
pub struct Session {
    pub id: u64,
    pub uuid: String,
    pub peer_connection: Arc<RTCPeerConnection>,
    pub broadcaster: Broadcaster,
} 

impl Session {

    pub async fn new() -> Result<Self> {
        // Create a MediaEngine
        let mut m = MediaEngine::default();
        // Register default codecs
        m.register_default_codecs();

        // Create a new API with the MediaEngine
        let api = APIBuilder::new().with_media_engine(m).build();

        // Define ICE servers
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // Create a new RTCPeerConnection
        let peer_connection = api.new_peer_connection(config).await.unwrap();

        Ok(Self {
            id: 0,
            uuid: uuid::Uuid::new_v4().to_string(),
            peer_connection: Arc::new(peer_connection),
            broadcaster: Broadcaster::new().await.unwrap(),
        })
    }
    
    pub async fn get_sdp_offer(&self) -> Result<String> {
        println!("->> {:<12} - get_sdp_offer", "Broadcaster");

        let offer = self.peer_connection.create_offer(None).await.unwrap();
        self.peer_connection.set_local_description(offer).await;


        let mut local_description = self.peer_connection.local_description().await.unwrap();
        Ok(local_description.sdp)
    }

    /// Sets an SDP answer
    pub async fn set_sdp_answer(&self, sdp: String) -> Result<()> {
        println!("->> {:<12} - set_sdp_answer", "Broadcaster");
        let remote_desc = RTCSessionDescription::answer(sdp)?;
        self.peer_connection.set_remote_description(remote_desc).await?;
        Ok(())
    }

    pub async fn connect(&self) -> Result<String> {

        // get sdp offer from broadcaster
        println!("->> {:<12} - connect", "Session");
        let sdp_offer = self.get_sdp_offer().await.unwrap();

        Ok(sdp_offer)
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

        session.broadcaster.add_audio_track("audio/opus", session.peer_connection.clone()).await.unwrap();
        // session.broadcaster.broadcast_audio_from_file("output2.ogg");

        println!("session created");

        sessions.push(Some(session.clone()));

        Ok(session)
    }

    pub async fn get_session(&self, id: u64) -> Result<Session> {
        let sessions = self.sessions.lock().await;
        let session = sessions.get(id as usize).and_then(|f| f.clone());
        session.ok_or(Error::SessionDeleteFailIdNotFound{ id })
    }
}

