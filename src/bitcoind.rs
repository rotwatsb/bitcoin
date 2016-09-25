use std::thread;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, VecDeque};

use bitcoin::network::encodable::{ConsensusEncodable, ConsensusDecodable};
use bitcoin::network::serialize::{RawEncoder, RawDecoder};
use bitcoin::network::message::NetworkMessage;
use bitcoin::network::message_blockdata::{GetHeadersMessage, Inventory, InvType};
use bitcoin::network::constants::Network;
use bitcoin::network::address::Address;
use bitcoin::blockdata::blockchain::Blockchain;
use bitcoin::util::Error;

use peerd::Peerd;
use util::{ThreadResponse, ipv4_to_ipv4addr, string_of_address};

pub const MAX_CNXS: usize = 50;
pub const MAX_BLCKS: usize = 1000;

pub struct Bitcoind {
    new_addresses: Arc<Mutex<Vec<Address>>>,
    active_connections: Arc<Mutex<HashMap<String, Sender<NetworkMessage>>>>,
    blockchain: Blockchain,
    path_to_chain: String,
}

pub enum State {
    Sync,
    Listen,
}

impl Bitcoind {
    pub fn new(ip: &str, port: u16, path_to_chain: &str) -> Bitcoind {

        let blockchain = 
            match File::open(path_to_chain) {
                Err(e) => {
                    println!("Count not open file at {}", path_to_chain);
                    Blockchain::new(Network::Bitcoin)
                },
                Ok(file) => {
                    let mut decoder = RawDecoder::new(BufReader::new(file));
                    match ConsensusDecodable::consensus_decode(&mut decoder) {
                        Err(e) => {
                            println!("Could not load blockchain at {}: {:?}",
                                     path_to_chain, e);
                            panic!("Couldn't load blockchain!");
                            Blockchain::new(Network::Bitcoin)
                        },
                        Ok(blckchn) => blckchn,
                    }
                },
            };
        
        let address = Address {
            services: 1,
            address: ipv4_to_ipv4addr(ip).to_ipv6_mapped().segments(),
            port: port,
        };
        
        Bitcoind {
            new_addresses: Arc::new(Mutex::new(vec![address])),
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            blockchain: blockchain,
            path_to_chain: path_to_chain.to_string(),
        }
    }

    fn start_connection_manager(&mut self)
                                -> Result<Receiver<ThreadResponse>, Error> {
        
        let (sm_sender, sm_receiver): (Sender<ThreadResponse>,
                                       Receiver<ThreadResponse>) = channel();
        
        let new_addresses = self.new_addresses.clone();
        let active_connections = self.active_connections.clone();
        thread::spawn(move || {
            loop {
                // access shared structures needed to initiate new connections
                let (mut new_addrs_vec, mut act_cnxs_map) =
                    (new_addresses.lock().unwrap(),
                     active_connections.lock().unwrap());

                while let Some(new_addr) = new_addrs_vec.pop() {
                    if act_cnxs_map.len() >= MAX_CNXS {
                        new_addrs_vec.push(new_addr);
                        break;
                    }
                    // initiate connection to new_addr
                    let ip_address = string_of_address(&new_addr);
                    
                    if act_cnxs_map.contains_key(&ip_address) == false {
                        let mut peerd = Peerd::new(ip_address.clone(), new_addr.port);
                        let (cnx_sender, cnx_receiver) = channel();
                        
                        if let Ok(peer_chan) = peerd.listen(cnx_receiver) {
                            act_cnxs_map.insert(ip_address, cnx_sender);
                            
                            let cm_addresses_arc = new_addresses.clone();
                            let cm_active_cnxs_arc = active_connections.clone();
                            let sm_sender_clone = sm_sender.clone();
                            thread::spawn(move || {
                                loop {
                                    match peer_chan.recv() {
                                        Ok(ThreadResponse::Addresses(mut addresses)) => {
                                            let mut new_addresses =
                                                cm_addresses_arc.lock().unwrap();
                                            while let Some((_, new_addr)) = addresses.pop() {
                                                new_addresses.push(new_addr);
                                            }
                                        },
                                        Ok(ThreadResponse::Headers(ip, headers)) => {
                                            sm_sender_clone.send(ThreadResponse::Headers(ip, headers));
                                        },
                                        Ok(ThreadResponse::Inv(ip, inventory)) => {
                                            sm_sender_clone.send(ThreadResponse::Inv(ip, inventory));
                                        },
                                        Ok(ThreadResponse::Block(block)) => {
                                            sm_sender_clone.send(ThreadResponse::Block(block));
                                        },
                                        Ok(ThreadResponse::Tx(transaction)) => {
                                            sm_sender_clone.send(ThreadResponse::Tx(transaction));
                                        }
                                        Ok(ThreadResponse::CloseThread((err, tx))) => {
                                            tx.send(());
                                            println!("{:?}", err);
                                            
                                            let mut active_cnxs =
                                                cm_active_cnxs_arc.lock().unwrap();
                                            active_cnxs.remove(&peerd.config.peer_addr);
                                            
                                            println!("Active connections: {}",
                                                     active_cnxs.len());
                                            
                                            break;
                                        },
                                        Err(e) => {
                                            println!("{:?}", e);
                                            break;
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
            }
        });
        Ok(sm_receiver)
    }

    fn save(&mut self) -> Result<(), Error> {
        match OpenOptions::new()
            .write(true)
            .create(true)
            .open(&self.path_to_chain) {
                Ok(file) => {
                    let mut encoder = RawEncoder::new(
                        BufWriter::new(file));
                    match self.blockchain.consensus_encode(&mut encoder) {
                        Ok(()) => println!("Done saving blockchain."),
                        Err(e) => {
                            println!("Faild to write to blockchian");
                            return Err(e);
                        },
                    }
                },
                Err(e) => println!("Could open blockchain for saving"),
            }
        Ok(())
    }
    
    pub fn listen(mut self) -> Result<(), Error> {
        
        let sm_receiver = try!(self.start_connection_manager());

        let mut state_queue: VecDeque<State> = VecDeque::new();
        state_queue.push_back(State::Sync);
        
        loop {
            match state_queue.pop_front() {
                Some(State::Sync) => {
                    // wait until connection pool is at least half full
                    loop {
                        let active_cnx_map = self.active_connections.lock().unwrap();
                        if active_cnx_map.len() >= MAX_CNXS / 2 {
                            break;
                        }
                    }

                    let mut done = false;
                    while !done {
                        println!("Headers sync from {:x}",
                                 self.blockchain.best_tip_hash());
                        let locator_hashes = self.blockchain.locator_hashes();
                        let msg = NetworkMessage::GetHeaders(GetHeadersMessage::new(
                            locator_hashes, Default::default()));
                        {
                            let active_cnx_map = self.active_connections.lock().unwrap();
                            for sender in active_cnx_map.values() {
                                sender.send(msg.clone());
                            }
                        }
                        let mut new_headers = false;
                        let mut some_count = 0;
                        let mut none_count = 0;
                        while !new_headers {
                            match sm_receiver.recv() {
                                Ok(ThreadResponse::Headers(ip, headers)) => {
                                    let mut no_headers = true;
                                    for lone_header in headers.iter() {
                                        no_headers = false;
                                        match self.blockchain.add_header(
                                            lone_header.header) {
                                            Err(Error::DuplicateHash) => (),
                                            Err(e) => println!("{:?}", e),
                                            Ok(()) => {
                                                new_headers = true;
                                            },
                                        }
                                    }
                                    if no_headers {
                                        none_count += 1;
                                    }
                                    else {
                                        some_count += 1;
                                    }
                                },
                                _ => ()
                            }
                            if none_count > some_count {
                                done = true;
                            }
                        }
                        if new_headers {
                            try!(self.save())
                        }
                    }
                    state_queue.push_back(State::Listen);
                },
                Some(State::Listen) => {
                    loop {
                        match sm_receiver.recv() {
                            Ok(ThreadResponse::Inv(ip, inventory)) => {
                                let mut inv_to_get: Vec<Inventory> = vec![];
                                for inv in inventory {
                                    if inv.inv_type == InvType::Block {
                                        inv_to_get.push(inv);
                                    }
                                }
                                if !inv_to_get.is_empty() {
                                    let active_cnx_map = self.active_connections.lock().unwrap();
                                    if let Some(sender) = active_cnx_map.get(&ip) {
                                        sender.send(NetworkMessage::GetData(inv_to_get));
                                    }
                                }
                            },
                            Ok(ThreadResponse::Block(block)) => {
                                //println!("Block received: {:?}", block);
                                println!("Block received");
                            },
                            _ => println!("Received some other message"),
                        }
                    }
                },
                None => {
                    break;
                }
            }
        }
        Ok(())
    }
}

