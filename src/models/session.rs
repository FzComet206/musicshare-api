use crate::utils::error::{Error, Result};
use crate::media::broadcaster::{
    BroadcasterHandle,
    BroadcasterCommand,
    BroadcasterEvent,
};
use crate::models::peer::PeerConnection;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::Path;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::fs;
use tokio::sync::oneshot;

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
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::api::media_engine::MIME_TYPE_OPUS;

use tokio::sync::mpsc;

use tokio::sync::broadcast;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

use crate::media::file_manager::{ FileManager, FMDownloadParams };
use crate::models::queue::PlayQueue;
use crate::models::queue::QueueAction::{ Next, Stop, Pass, NotFound };
use crate::media::broadcaster::Broadcaster;

#[derive(Clone, Debug)]
pub struct Session {
    pub uuid: String,
    pub peer_connections: Arc<Mutex<HashMap<String, PeerConnection>>>,
    pub broadcaster: BroadcasterHandle,
    pub queue: Arc<Mutex<PlayQueue>>,
    pub update: Arc<Mutex<broadcast::Sender<String>>>,
    // later add user id and time lapsed
} 

impl Session {
    pub async fn new(
        session_id: String, 
        broadcaster_handle: BroadcasterHandle,
        peer_connections: Arc<Mutex<HashMap<String, PeerConnection>>>,
    ) -> Result<Self> {

        let session = Self {
            uuid: session_id,
            peer_connections, 
            broadcaster: broadcaster_handle,
            queue: Arc::new(Mutex::new(PlayQueue::new())),
            update: Arc::new(Mutex::new(broadcast::channel(100).0)),
        };

        session.autoplay_loop().await?;

        Ok(session)
    }

    pub async fn create_peer(&mut self) -> Result<(String, oneshot::Receiver<()>)> {

        let mut pc = PeerConnection::new().await;
        let uuid = pc.uuid.clone();
        println!("->> {:<12} - {} - create_peer", "Session", uuid);


        let mut peer_connections = self.peer_connections.lock().await;
        peer_connections.insert(uuid.clone(), pc);

        // send peer uuid to broadcaster for attaching track
        let (tx, rx) = oneshot::channel();
        let broadcaster = self.broadcaster.clone();
        let _uuid = uuid.clone();

        let handle = tokio::spawn(async move {
            broadcaster.cmd_tx.send(
                BroadcasterCommand::Attach { 
                    peer_id: _uuid.clone(),
                    reply: tx
                }
            ).await.map_err(|e| {
                Error::BroadcasterError { msg: "Failed to attach track to broadcaster".to_string() }
            });
        });

        Ok((uuid, rx))
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
                self.play(key).await?;
                self.ping(queue.get_id()).await?;
            },
            Pass => self.ping(queue.get_id()).await?,
            _ => self.ping(queue.get_id()).await?,
        }
        Ok(())
    }

    pub async fn remove_from_queue(&self, key: String) -> Result<()> {
        let mut queue = self.queue.lock().await;
        match queue.remove(key) {
            Next(key) => {
                self.play(key).await?;
                self.ping(queue.get_id()).await?;
            },
            Stop => {
                self.clean_active_file().await?;
                self.ping(queue.get_id()).await?;
            },
            NotFound => {
                return Err(Error::QueueError { msg: "Key not found".to_string() });
            },
            Pass => { 
                self.ping(queue.get_id()).await?;
            }
        }
        Ok(())
    }

    pub async fn play(&self, key: String) -> Result<()> {

        let sender = self.update.clone();
        let queue = self.queue.clone();

        tokio::spawn(async move {
            sender.lock().await.send(queue.lock().await.get_id());
        });

        self.broadcaster.cmd_tx.send(BroadcasterCommand::Stop).await
            .map_err(|e| { Error::BroadcasterError { msg: "Failed to stop broadcaster".to_string() }})?;

        // start playing the new file 
        self.broadcaster.cmd_tx.send(BroadcasterCommand::Play { key: key.clone() }).await
            .map_err(|e| { Error::BroadcasterError { msg: "Failed to play file".to_string() }})?;

        Ok(())
    }

    pub async fn autoplay_loop(&self) -> Result<()> {

        // let mut event_rx = self.broadcaster.event_rx.lock().await;
        let broadcaster = self.broadcaster.clone();
        let queue = self.queue.clone();
        let sender = self.update.clone();


        tokio::spawn(async move {

            let mut event_rx = broadcaster.event_rx.lock().await;

            while let Some(event) = event_rx.recv().await {
                match event {
                    BroadcasterEvent::End => {
                        // handle the next item in the queue
                        let next_key = queue.lock().await.next();
                        sender.lock().await.send(queue.lock().await.get_id());
                        if !next_key.is_empty() {
                            let _ = broadcaster
                                .cmd_tx
                                .send(BroadcasterCommand::Play { key: next_key })
                            .await;
                        } else {
                            println!("Empty queue, stopping autoplay loop");
                        }

                    },
                    _ => (),
                }
            }
            println!("->> {:<12} - {} - end of autoplay loop", "Session", "autoplay_loop");
        });

        Ok(())
    }

    pub async fn clean_active_file(&self) -> Result<()> {

        self.broadcaster.cmd_tx.send(
            BroadcasterCommand::Stop
        ).await.map_err(|e| {
            Error::BroadcasterError { msg: "Failed to stop broadcaster".to_string() }
        })?;

        let session_dir = format!("./sessions/{}", self.uuid.clone());
        let mut entries = match fs::read_dir(session_dir.clone()).await {
            Ok(entries) => entries,
            Err(e) => return Err(Error::ResetFileError { msg: e.to_string() }),
        };

        while let Some(entry) = entries.next_entry().await.transpose() {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(path).await?;
            }
        }
        Ok(())
    }

    // server side events
    pub async fn get_sender(&self) -> Result<broadcast::Sender<String>> {
        let update = self.update.lock().await;
        Ok(update.clone())
    }

    pub async fn ping(&self, index: String) -> Result<()> {
        let sender = self.update.lock().await;

        sender.send(index).map_err(|e| {
            Error::SSEError { msg: e.to_string() }
        })?;

        println!("Pinged Queue Update");
        Ok(())
    }
}


#[derive(Clone, Debug)]
pub struct SessionController{
    pub sessions: Arc<Mutex<HashMap<String, Option<Session>>>>,

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
        let session_id = uuid::Uuid::new_v4().to_string();

        // init control handles
        let (cmd_tx, mut cmd_rx) = mpsc::channel(100);
        let (event_tx, mut event_rx) = mpsc::channel(100);


        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                ..Default::default()
            },
            "audio".to_owned(),
            "broadcaster".to_owned(),
        ));

        let peer_connections = Arc::new(Mutex::new(HashMap::new()));

        // spin up the broadcaster
        let broadcaster = Broadcaster::new(track, cmd_rx, event_tx, Arc::clone(&peer_connections), session_id.clone()).await?;
        tokio::spawn(async move {
            broadcaster.run().await;
        });

        let broadcaster_handle = BroadcasterHandle {
            cmd_tx,
            event_rx: Arc::new(Mutex::new(event_rx))
        };
        // create broadcaster and spin it on a task
        let mut session = Session::new(session_id.clone(), broadcaster_handle, Arc::clone(&peer_connections)).await?;

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id.clone(), Some(session.clone()));

        Ok(session_id)
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