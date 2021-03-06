use std::time::Duration;
use std::thread;
use std::sync::mpsc::{Sender, Receiver, channel, TryRecvError};

use bitcoin::network::listener::Listener;
use bitcoin::network::constants::Network;
use bitcoin::network::socket::Socket;
use bitcoin::network::message::{SocketResponse, NetworkMessage};

use util::ThreadResponse;

#[derive(Clone)]
pub struct Peerd {
    pub config: NetworkConfig,
}

impl Peerd {
    pub fn new(ip: String, port: u16) -> Peerd {
        Peerd {
            config: NetworkConfig::new(Network::Bitcoin, ip, port),
        }
    }
    
    pub fn listen(&mut self, master: Receiver<NetworkMessage>)
                  -> Result<Receiver<ThreadResponse>, String> {
        let (sender, receiver): (Sender<ThreadResponse>,
                                 Receiver<ThreadResponse>) = channel();
        
        let self_clone = self.clone();
        
        thread::spawn(move || {
            println!("Trying to connect to {}", self_clone.config.peer_addr);
            match self_clone.loop_connect() {
                Ok((net_chan, mut sock)) => {
                    println!("Connected to {}", self_clone.config.peer_addr);
                    
                    loop {
                        match master.try_recv() {
                            Ok(msg) => {
                                match sock.send_message(msg) {
                                    Ok(()) => (),
                                    Err(e) => println!("Failed to send message: {:?}", e),
                                }
                            },
                            Err(TryRecvError::Empty) => (),
                            Err(TryRecvError::Disconnected) => {
                                println!("Channel disconnected");
                            }
                        }
                        match net_chan.recv() {
                            Ok(SocketResponse::MessageReceived(msg)) => {
                                match msg {
                                    NetworkMessage::Version(version) => {
                                        println!("Message received: Version");
                                        match sock.send_message(NetworkMessage::Verack) {
                                            Ok(()) => (),
                                            Err(e) => println!("Failed to send verack message: {:?}", e),
                                        }
                                    },
                                    NetworkMessage::Verack => {
                                        println!("Message received: Verack");
                                        match sock.send_message(NetworkMessage::GetAddr) {
                                            Ok(()) => (),
                                            Err(e) => println!("Failed to send getaddr message: {:?}", e),
                                        }
                                    },
                                    NetworkMessage::Addr(addresses) => {
                                        println!("Message received: Addresses");
                                        sender.send(ThreadResponse::Addresses(addresses)).unwrap();
                                    },
                                    NetworkMessage::Ping(nonce) => {
                                        println!("Message received: Ping");
                                        match sock.send_message(NetworkMessage::Pong(nonce)) {
                                            Ok(()) => (),
                                            Err(e) => println!("Failed to send pong response to ping: {:?}", e),
                                        }
                                    },
                                    NetworkMessage::Pong(nonce) => {
                                        println!("Message received: Pong");
                                    },
                                    NetworkMessage::Inv(inventory) => {
                                        println!("Message received: Inventory");
                                        sender.send(ThreadResponse::Inv(
                                            self_clone.peer().to_string(),
                                            inventory)).unwrap();
                                    },
                                    NetworkMessage::GetData(inventory) => {
                                        println!("Message received: GetData");
                                    },
                                    NetworkMessage::NotFound(inventory) => {
                                        println!("Message received: NotFound");
                                    },
                                    NetworkMessage::GetBlocks(blockdata) => {
                                        println!("Message received: GetBlocks");
                                    },
                                    NetworkMessage::GetHeaders(blockdata) => {
                                        println!("Message received: GetHeaders");
                                    },
                                    NetworkMessage::MemPool => {
                                        println!("Message received: MemPool");
                                    },
                                    NetworkMessage::Tx(transaction) => {
                                        println!("Message received: Tx");
                                        sender.send(ThreadResponse::Tx(transaction)).unwrap();
                                    },
                                    NetworkMessage::Block(block) => {
                                        println!("Message received: Block");
                                        sender.send(ThreadResponse::Block(block)).unwrap();
                                    },
                                    NetworkMessage::Headers(lone_block_headers) => {
                                        println!("Message received: Headers");
                                        sender.send(ThreadResponse::Headers(
                                            self_clone.peer().to_string(),
                                            lone_block_headers)).unwrap();
                                    },
                                    NetworkMessage::GetAddr => {
                                        println!("Message received: GetAddr");
                                    },
                                }
                            },
                            Ok(SocketResponse::ConnectionFailed(err, tx)) => {
                                tx.send(()); // tear down failing thread
                                let(txx, rxx) = channel(); // notify parent thread of failure
                                sender.send(ThreadResponse::CloseThread((format!("{:?}", err), txx)));
                                rxx.recv().unwrap(); // tear down this thread
                                break;
                            },
                            Err(_) => {
                                println!("YIKES!");
                            },
                        }
                    }
                },
                Err(e) => {
                    println!("Could not connect to {}", self_clone.config.peer_addr);
                    let(txx, rxx) = channel(); // notify parent thread of failure
                    sender.send(ThreadResponse::CloseThread((e, txx)));
                    rxx.recv().unwrap(); // tear down this thread
                },
            }
        });
        Ok(receiver)
    }
        
    fn loop_connect(&self) -> Result<(Receiver<SocketResponse>, Socket), String> {
        let max_attempts = 3;
        let mut err: String = "".to_string();
        for _ in 0..max_attempts {
            match self.start() {
                Ok((chan, sock)) => { return Ok((chan, sock)); }
                Err(e) => { err = format!("Loop connect error: {:?}", e); }
            }
            thread::sleep(Duration::from_secs(3));
        }
        Err(err)
    }
}

impl Listener for Peerd {
    fn peer<'a>(&'a self) -> &'a str {
        &(self.config.peer_addr)
    }

    fn port(&self) -> u16 {
        self.config.peer_port
    }

    fn network(&self) -> Network {
        self.config.network
    }
}

#[derive(Clone)]
pub struct NetworkConfig {
    /// The network this configuration is for
    pub network: Network,
    /// Address to connect to the network peer on
    pub peer_addr: String,
    /// Port to connect to the network peer on
    pub peer_port: u16,
}

impl NetworkConfig {
    fn new(network: Network, peer_addr: String,
           peer_port: u16) -> NetworkConfig {
        NetworkConfig {
            network: network,
            peer_addr: peer_addr,
            peer_port: peer_port,
        }
    }
}
