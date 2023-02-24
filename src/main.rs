use std::{str, io::Error, sync::{Arc, Mutex}, collections::HashMap, net::SocketAddr};

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, StreamExt, TryStreamExt, SinkExt, pin_mut};
use log::info;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_tungstenite::tungstenite::Message;

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let _ = env_logger::try_init();
    
    let state = PeerMap::new(Mutex::new(HashMap::new()));
    
    let try_socket = TcpListener::bind("127.0.0.1:8888").await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: localhost:8888");
    
    tokio::spawn(listen_udp(state.clone()));
    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(state.clone(), stream, addr));
    }
    
    Ok(())
}

async fn handle_connection(peer_map: PeerMap, raw_stream: TcpStream, addr: SocketAddr) {
    println!("Incoming TCP connection from: {}", addr);
    
    // Create a WebSocket Stream
    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    info!("New WebSocket connection: {}", addr);
    
    // Splitting the WebSocket stream into outgoing and incoming streams
    let (mut outgoing, incoming) = ws_stream.split();
    
    // Creating a Message Channel to send messages to the connected WebSockets
    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx);
    
    let receiving = rx.map(Ok).forward(outgoing);
    pin_mut!(receiving);
    receiving.await;
    
    println!("{} Disconnected", &addr);
    peer_map.lock().unwrap().remove(&addr);
}

async fn listen_udp(peer_map: PeerMap) {
    let socket = UdpSocket::bind("127.0.0.1:8889")
        .await
        .expect("Failed to bind socket");
    info!("Listening on {}", socket.local_addr().unwrap());
    
    loop {
        let mut buf = [0; 512];
        socket.recv(&mut buf)
            .await
            .expect("Failed to receive message");
        
        let peers = peer_map.lock().unwrap();
        
        for (_, peer) in peers.iter() { 
            let msg = match str::from_utf8(&buf) {
                Ok(s) => s,
                Err(e) => panic!("{}", e)
            };
            
            peer.unbounded_send(Message::Text(msg.into())).unwrap();
        }
    }
}