use crate::utils::error::{Error, Result};
use crate::media::broadcaster::Broadcaster;
use crate::models::peer::PeerConnection;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::Path;
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

use tokio::sync::broadcast;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{config::Region, Client};

use crate::media::file_manager::{ FileManager, FMDownloadParams };
use crate::models::queue::PlayQueue;
use crate::models::queue::QueueAction::{ Next, Stop, Pass, NotFound };


#[derive(Clone, Debug)]
pub struct Session {
    pub id: u64,
    pub uuid: String,
    pub peer_connections: Arc<Mutex<HashMap<String, PeerConnection>>>,
    pub broadcaster: Broadcaster,
    pub queue: Arc<Mutex<PlayQueue>>,
    pub update: Arc<Mutex<broadcast::Sender<String>>>,
    pub s3_client: Client,
    // later add user id and time lapsed
} 

impl Session {
    pub async fn new() -> Result<Self> {

        let broadcaster = Broadcaster::new().await?;

        // s3 client for downloading single file only
        let region_provider = RegionProviderChain::first_try(Region::new("us-east-1"));
        let region = region_provider.region().await.unwrap();
        let shared_config = aws_config::from_env().region(region_provider).load().await;
        let client = Client::new(&shared_config);

        Ok(Self {
            id: 0,
            uuid: uuid::Uuid::new_v4().to_string(),
            peer_connections: Arc::new(Mutex::new(HashMap::new())),
            broadcaster: broadcaster,
            queue: Arc::new(Mutex::new(PlayQueue::new())),
            update: Arc::new(Mutex::new(broadcast::channel(100).0)),
            s3_client: client,
        })
    }

    pub async fn create_peer(& mut self) -> Result<(String)> {

        let mut peer_connections = self.peer_connections.lock().await;
        let mut pc = PeerConnection::new(self.broadcaster.get_track().await.unwrap()).await;

        // uuid for identification
        let uuid = pc.uuid.clone();

        println!("->> {:<12} - {} - create_peer", "Session", uuid);

        peer_connections.insert(uuid.clone(), pc);
        Ok(uuid)
    }

    pub async fn get_offer(&self, peerid: String) -> Result<String> {

        println!("->> {:<12} - {} - get_sdp_offer", "Session", peerid);

        let mut peer_connections = self.peer_connections.lock().await;

        let offer = peer_connections.get_mut(&peerid).unwrap().get_offer().await?;
        Ok(offer)
    }

    pub async fn set_answer(&self, sdp: String, peerid: String) -> Result<()> {

        println!("->> {:<12} - {} - set_answer", "Session", peerid);
        let mut peer_connections = self.peer_connections.lock().await;
        let pc = peer_connections.get_mut(&peerid).unwrap().set_answer(sdp).await?;
        Ok(())
    }

    pub async fn get_ice(&self, peerid: String) -> Result<Vec<RTCIceCandidate>> {

        println!("->> {:<12} - {} - get_ice", "Session", peerid);

        let mut peer_connections = self.peer_connections.lock().await;
        let ice = peer_connections.get_mut(&peerid).unwrap().get_ice().await?;
        Ok(ice)
    }

    pub async fn add_ice(&self, candidate: RTCIceCandidateInit, peerid: String) -> Result<()> {

        println!("->> {:<12} - {} - add_ice", "Session", peerid);
        let mut peer_connections = self.peer_connections.lock().await;
        let ice = peer_connections.get_mut(&peerid).unwrap().add_ice(candidate).await?;
        Ok(())
    }

    pub async fn get_peers(&self) -> Result<Vec<String>> {
        let peer_connections = self.peer_connections.lock().await;

        let mut peers = Vec::new();

        for (uuid, pc) in peer_connections.iter() {
            let active = pc.active.lock().await;
            peers.push(format!("uuid: {} -- active: {}", uuid, *active));
        }
        Ok(peers)
    }

    pub async fn get_queue(&self) -> Result<Vec<Vec<String>>> {
        let queue = self.queue.lock().await;
        Ok(queue.get_all())
    }

    // queue change opreations pass in a function call back
    pub async fn add_to_queue(&self, key: String, title: String) -> Result<()> {
        let mut queue = self.queue.lock().await;
        match queue.add(key, title) {
            Next(key) => {
                self.set_active_file(key).await?;
            },
            Pass => (),
            _ => ()
        }
        self.ping().await?;
        Ok(())
    }

    pub async fn remove_from_queue(&self, key: String) -> Result<()> {
        let mut queue = self.queue.lock().await;
        match queue.remove(key) {
            Next(key) => {
                self.set_active_file(key).await?;
            },
            Stop => {
                todo!()
            },
            NotFound => {
                return Err(Error::QueueError { msg: "Key not found".to_string() });
            },
            Pass => ()
        }
        self.ping().await?;
        Ok(())
    }

    pub async fn reorder_queue(&self, key: String, new_index: u64) -> Result<()> {
        let mut queue = self.queue.lock().await;
        match queue.reorder(key, new_index as usize) {
            Next(key) => {
                // handle the next item in the queue
                self.set_active_file(key).await?;
            },
            NotFound => {
                return Err(Error::QueueError { msg: "Key not found".to_string() });
            },
            Pass => (),
            _ => ()
        }
        self.ping().await?;
        Ok(())
    }

    pub async fn set_active_file(&self, key: String) -> Result<()> {
        // sets the active file for broadcaster to play

        // genenerate a session directory in ./sessions/session_id

        let session_id = self.uuid.clone();
        let session_dir = format!("./sessions/{}", session_id.clone());

        // Ensure the output directory exists
        if !Path::new(&session_dir).exists() {
            tokio::fs::create_dir_all(session_dir.clone()).await?;
        }
        
        let file_dir = format!("{}/{}.ogg", session_dir, key);

        let mut file = File::create(file_dir).map_err(|err| {
            Error::S3DownloadError { msg: "Failed to initialize file for s3 download".to_string() }
        })?;

        let mut object = self.s3_client
            .get_object()
            .bucket("antaresmusicshare")
            .key(format!("{}.ogg", key))
            .send()
            .await
            .map_err(
                |e| {
                    Error::S3DownloadError { msg: e.to_string() }
                }
            )?;
        
        // download the file to the session directory with file manager
        let mut byte_count = 0_usize;
        while let Some(bytes) = object.body.try_next().await.map_err(|err| {
            Error::S3DownloadError { msg: "Failed to read from s3 download streams".to_string() }
        })? {
            let bytes_len = bytes.len();
            file.write_all(&bytes).map_err(|err| {
                Error::S3DownloadError { msg: "Failed to write from s3 stream to local file".to_string() }
            })?;
            byte_count += bytes_len;
        }

        let file_path = format!("{}/{}.ogg", session_dir, key);
        // invode broadcaster to play the filej
        self.broadcaster.broadcast_audio_from_file(&file_path).await?;

        println!("->> {:<12} - {} - set_active_file", "Session", key);
        Ok(())
    }

    pub async fn clean_actve_file(&self) -> Result<()> {
        // cleans the active files in session directory
        Ok(())
    }

    pub async fn get_sender(&self) -> Result<broadcast::Sender<String>> {
        let update = self.update.lock().await;
        Ok(update.clone())
    }

    pub async fn ping(&self) -> Result<()> {
        let sender = self.update.lock().await;

        sender.send("check".to_string()).map_err(|e| {
            Error::SSEError { msg: e.to_string() }
        })?;

        println!("Pinged Queue Update");
        Ok(())
    }
}


#[derive(Clone, Debug)]
pub struct SessionController{
    pub sessions: 
        Arc<
            Mutex<
                HashMap<
                    String, Option<Session>
                    >>>,
    pub file_manager: Arc<Mutex<FileManager>>,
}

impl SessionController{

    pub async fn new() -> Result<Self> {
        Ok(Self {
            sessions: Arc::default(),
            file_manager: Arc::new(Mutex::new(FileManager::new().await?)),
        })
    }

    pub async fn create_session(&self) -> Result<(String)> {

        println!("->> {:<12} - create_session", "Controller");

        let mut sessions = self.sessions.lock().await;
        let mut session = Session::new().await?;

        let uuid = uuid::Uuid::new_v4().to_string();
        sessions.insert(uuid.clone(), Some(session.clone()));

        Ok(uuid)
    }

    pub async fn get_session(&self, id: String) -> Result<Session> {
        let sessions = self.sessions.lock().await;
        let session = sessions.get(&id.to_string()).and_then(|f| f.clone());
        session.ok_or(Error::SessionNotFound { id })
    }

    pub async fn get_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.sessions.lock().await;
        Ok(sessions.values().filter_map(|f| f.clone()).collect())
    }

    pub async fn get_file_manager(&self) -> Result<FileManager> {
        let file_manager = self.file_manager.lock().await;
        Ok(file_manager.clone())
    }

    pub async fn get_sender_with_id(&self, id: String) -> Result<broadcast::Sender<String>> {
        let file_manager = self.file_manager.lock().await;
        Ok(file_manager.get_sender_with_id(id).await?)
    }

    pub async fn add_sender_with_id(&self, id: String, sender: broadcast::Sender<String>) -> Result<()> {
        let mut file_manager = self.file_manager.lock().await;
        file_manager.add_sender_with_id(id, sender).await?;
        Ok(())
    }
}

