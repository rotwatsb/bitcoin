use std::thread;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

use bitcoin::network::encodable::{ConsensusEncodable, ConsensusDecodable};
use bitcoin::network::serialize::{RawEncoder, RawDecoder, BitcoinHash};
use bitcoin::network::message::NetworkMessage;
use bitcoin::network::message_blockdata::{GetHeadersMessage, Inventory, InvType};
use bitcoin::network::constants::Network;
use bitcoin::network::address::Address;
use bitcoin::blockdata::blockchain::Blockchain;
use bitcoin::blockdata::block::{Block, BlockHeader};
use bitcoin::util::address::Address as Secp256k1Address;
use bitcoin::util::hash::Sha256dHash;
use bitcoin::util::Error;

use postgres::{Connection, SslMode};

use peerd::Peerd;
use util::{ThreadResponse, ipv4_to_ipv4addr, string_of_address, addr_from_hash};

pub const MAX_CNXS: usize = 50;
pub const MAX_BLCKS: usize = 200;

pub struct Bitcoind {
    new_addresses: Arc<Mutex<Vec<Address>>>,
    active_connections: Arc<Mutex<HashMap<String, Sender<NetworkMessage>>>>,
    blockchain: Blockchain,
    db_cnx: String,
    db_state: VecDeque<Sha256dHash>,
    path_to_chain: String,
}

pub enum State {
    Sync,
    Listen,
}

impl Bitcoind {
    pub fn new(ip: &str, port: u16, path_to_chain: String, db_cnx: String)
               -> Bitcoind {

        let address = Address {
            services: 1,
            address: ipv4_to_ipv4addr(ip).to_ipv6_mapped().segments(),
            port: port,
        };
        
        Bitcoind {
            new_addresses: Arc::new(Mutex::new(vec![address])),
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            blockchain: load_blockchain(&path_to_chain),
            db_cnx: db_cnx,
            db_state: load_db_state(&path_to_chain).iter()
                .map(|&hash| hash.clone())
                .collect::<VecDeque<Sha256dHash>>(),
            path_to_chain: path_to_chain,
        }
    }

    fn start_connection_manager(&mut self)
                                -> Result<Receiver<ThreadResponse>, String> {
        
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
                                        },
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
    
    pub fn listen(mut self) -> Result<(), String> {
        
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
                                        done = true;
                                        break;
                                    }
                                },
                                Err(e) => println!("Error syncing: {:?}", e),
                                _ => (),
                            }
                        }
                        if new_headers {
                            try!(self.save_blockchain())
                        }
                    }
                    println!("SYNCED!");
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
                                let block_clone = block.clone();
                                match self.blockchain.add_block(block) {
                                    Ok(()) => {
                                        try!(self.update_db(block_clone));
                                    },
                                    Err(e) => {
                                        match e {
                                            Error::PrevHashNotFound => 
                                                println!("Prev hash not found when adding block!"),
                                            Error::DuplicateHash => (),
                                            
                                            _ => println!("UNExpected error adding block!"),
                                        }
                                    },
                                }
                                println!("Block received");
                            },
                            Ok(ThreadResponse::Headers(ip, headers)) => {
                                println!("More headers received");
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

    fn remove_old_blocks(&mut self, conn: &Connection) -> Result<(), String> {
        fn remove_old_block(conn: &Connection, old_block_hash_string: &String)
                            -> Result<(), String> {
            
            // remove comments
            match conn.execute("DELETE FROM talk_comment WHERE block_hash_id = $1",
                               &[&old_block_hash_string]) {
                Ok(n) => println!("Successfully removed comment data: {}", n),
                Err(e) => return Err(format!("Could not remove comment data: {:?}", e)),
            }

            // set 'prev_block_hash_id' to NULL for old block's successor
            match conn.execute("UPDATE talk_block SET prev_block_hash_id = NULL \
                                WHERE block_hash IN (SELECT block_hash FROM \
                                talk_block WHERE prev_block_hash_id = $1)",
                               &[&old_block_hash_string]) {
                Ok(n) => println!("Successfully set 'prev_block_hash_id' to NULL \
                                   for block successor in database"),
                Err(e) => return Err(format!("Could not set 'prev_block_hash_id' to NULL for block successor in database: {:?}", e)),
            }

            // set news txins referencing doomed txouts to NULL
            match conn.execute("UPDATE talk_txin SET output_id = NULL \
                                WHERE output_id IN (SELECT output FROM talk_txout \
                                WHERE tx_id IN (SELECT tx_hash FROM \
                                talk_transaction WHERE block_hash_id = $1))",
                               &[&old_block_hash_string]) {
                Ok(n) => println!("Successfully nulled references to doomed txouts"),
                Err(e) => return Err(format!("Could not null references to doomed \
                                              txouts: {:?}", e)),
            }

            // remove txout data
            match conn.execute("DELETE FROM talk_txout WHERE tx_id IN (SELECT \
                                tx_hash FROM talk_transaction WHERE block_hash_id = \
                                $1)",
                               &[&old_block_hash_string]) {
                Ok(n) => println!("Successfully removed TxOut data: {}", n),
                Err(e) => return Err(format!("Could not remove TxOut data: {:?}", e)),
            }

            // remove txin data
            match conn.execute("DELETE FROM talk_txin WHERE tx_id IN (SELECT \
                                tx_hash FROM talk_transaction WHERE block_hash_id = \
                                $1)",
                               &[&old_block_hash_string]) {
                Ok(n) => println!("Successfully removed TxIn data: {}", n),
                Err(e) => return Err(format!("Could not remove TxIn data: {:?}", e)),
            }

            // remove transactions
            match conn.execute("DELETE FROM talk_transaction WHERE block_hash_id \
                                = $1", &[&old_block_hash_string]) {
                Ok(n) => println!("Successfully removed Transaction data: {}", n),
                Err(e) => return Err(format!("Could not remove Transaction data: {:?}", e)),
            }

            // delete the old block
            match conn.execute("DELETE FROM talk_block WHERE block_hash = $1",
                               &[&old_block_hash_string]) {
                Ok(n) => println!("Successfully removed old block: {}", n),
                Err(e) => return Err(format!("Could not remove old block: {:?}", e)),
            }

            Ok(())
        }

        // remove old blocks
        while self.db_state.len() > MAX_BLCKS {
            match self.db_state.pop_front() {
                Some(old_block_hash) => {
                    let old_block_hash_string = old_block_hash.be_hex_string();
                    try!(remove_old_block(&conn, &old_block_hash_string));
                    match self.blockchain.remove_txdata(old_block_hash) {
                        Ok(()) => (),
                        Err(e) => return Err(format!("Failed removing txdata from blockchain: {:?}", e)),
                    }
                }
                None => panic!("Failed to pop from block queue"),
            }
        }

        Ok(())
    }

    fn insert_block(&mut self, conn: &Connection, block: &Block,
                    block_hash: &Sha256dHash) -> Result<(), String> {
        
        fn insert_header(conn: &Connection, block: &Block,
                         block_hash_string: &String, block_height: u32,
                         prev_block_hash_option: &Option<String>)
                         -> Result<(), String> {

            match conn.execute("INSERT INTO talk_block (block_hash, \
                                prev_block_hash_id, block_size, block_height, \
                                merkleroot, time, median_time, bits, nonce) \
                                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                               &[block_hash_string,
                                 prev_block_hash_option,
                                 &(1111 as i32),
                                 &(block_height as i32),
                                 &block.header.merkle_root.be_hex_string(),
                                 &(block.header.time as i32),
                                 &(1111 as i32),
                                 &(block.header.bits as i64),
                                 &(block.header.nonce as i64)]) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Error writing header to database: {:?}", e)),
            }
        }

        fn insert_txs(conn: &Connection, block: &Block, block_hash_string: &String)
                      -> Result<(), String> {
            for tx in block.txdata.clone() {
                let tx_hash_string = tx.bitcoin_hash().be_hex_string();
                
                // insert transaction
                match conn.execute("INSERT INTO talk_transaction \
                                    (tx_hash, block_hash_id) VALUES ($1, $2)",
                                   &[&tx_hash_string, block_hash_string]) {
                    Ok(_) => (),
                    Err(e) => return Err(format!("Error writing transaction to database: {:?}", e)),
                }

                // insert inputs
                for input in tx.input {
                    let output_id: String = input.prev_hash.be_hex_string() +
                        &(input.prev_index.to_string());
                    match conn.execute("INSERT INTO talk_txin (tx_id, output_id) \
                                        VALUES ($1, (SELECT (CASE WHEN EXISTS \
                                        (SELECT 1 FROM talk_txout WHERE output=$2) \
                                        THEN $2 ELSE NULL END)))",
                                       &[&tx_hash_string,
                                         &output_id]) {
                        Ok(_) => (),
                        Err(e) =>  return Err(format!("Error writing input to database: {:?}", e)),
                    }
                }

                // insert outputs
                for (i, output) in tx.output.iter().enumerate() {
                    let output_id: String = tx_hash_string.clone() +
                        &(i.to_string());
                    
                    let v = output.script_pubkey.clone().into_vec();
                    let mut j = 0;
                    let l = v.len();
                    while j < l && v[j] != 20 { j += 1 }
                    let addr = if v.len() >= j + 21 {
                        addr_from_hash(&v[j + 1..j + 21])
                    } else { "".to_string() };

                    // insert address
                    match conn.execute("INSERT INTO talk_address (address) \
                                        (SELECT $1::VARCHAR WHERE NOT EXISTS \
                                        (SELECT $1::VARCHAR FROM talk_address WHERE \
                                        address=$1))",
                                       &[&addr]) {
                        Ok(_) => (),
                        Err(e) => return Err(format!("Error writing address to database: {:?}", e)),
                    }

                    // actually insert output (after inserting address)
                    match conn.execute("INSERT INTO talk_txout (tx_id, value, \
                                        output_index, address_id, output) \
                                        VALUES ($1, $2, $3, $4, $5)",
                                       &[&tx_hash_string,
                                         &(output.value as i64),
                                         &(i as i32),
                                         &addr,
                                         &output_id]) {
                        Ok(_) => (),
                        Err(e) => return Err(format!("Error writing txout to rainbow: {:?}", e)),
                    }
                }
            }
            Ok(())
        }

        let block_hash_string: String = block_hash.be_hex_string();
        println!("block hash: {}", block_hash_string.clone());

        let block_height: u32 =
            if let Some(block_node_ref) =
            self.blockchain.get_block(block_hash.clone()) {
                block_node_ref.height
            } else { 1111 };

        let prev_block_hash_option: Option<String> =
            if let Some(pbh) = self.db_state.iter()
            .find(|&&x| x == block.header.prev_blockhash) {
                Some(pbh.be_hex_string())
            } else { None };
        
        try!(insert_header(conn, block, &block_hash_string, block_height,
                           &prev_block_hash_option));
        try!(insert_txs(conn, block, &block_hash_string));
        Ok(())
    }

    fn update_db(&mut self, block: Block) -> Result<(), String> {
                
        let block_hash: Sha256dHash = block.header.bitcoin_hash();
        self.db_state.push_back(block_hash);
        let conn = Connection::connect(self.db_cnx.as_str(), SslMode::None).unwrap();
        try!(self.remove_old_blocks(&conn));
        try!(self.insert_block(&conn, &block, &block_hash));
        //self.save_scriptsigs(&block);
        try!(self.save_db_state());
        
        Ok(())
    }
    
    fn save_scriptsigs(&self, block: &Block) {
        let path_to_scriptsigs = beside_path(&self.path_to_chain, "scriptsigs.txt");
        let path_to_scriptpubkeys = beside_path(&self.path_to_chain, "scriptpubkeys.txt");
        
        match OpenOptions::new()
            .write(true)
            .create(true)
            .open(path_to_scriptsigs) {
                Ok(file) => {
                    let mut writer = BufWriter::new(file);
                    for tx in block.txdata.clone() {
                        for input in tx.input {
                            for byte in input.script_sig.clone().into_vec() {
                                writer.write(byte.to_string().as_bytes());
                                writer.write(" ".to_string().as_bytes());
                            }
                            writer.write("\n".to_string().as_bytes());
                            writer.write(format!("{}\n", input.script_sig).as_bytes());
                        }
                    }
                },
                Err(e) => println!("Could not open inputs for saving"),
            }

        match OpenOptions::new()
            .write(true)
            .create(true)
            .open(path_to_scriptpubkeys) {
                Ok(file) => {
                    let mut writer = BufWriter::new(file);
                    for tx in block.txdata.clone() {
                        for output in tx.output {
                            for byte in output.script_pubkey.clone().into_vec() {
                                writer.write(byte.to_string().as_bytes());
                                writer.write(" ".to_string().as_bytes());
                            }
                            writer.write("\n".to_string().as_bytes());
                            writer.write(format!("{}\n", output.script_pubkey).as_bytes());
                        }
                    }
                },
                Err(e) => println!("Could not open outputs for saving"),
            }
    }

    fn save_blockchain(&mut self) -> Result<(), String> {
        match OpenOptions::new()
            .write(true)
            .create(true)
            .open(&self.path_to_chain) {
                Ok(file) => {
                    let mut encoder = RawEncoder::new(
                        BufWriter::new(file));
                    match self.blockchain.consensus_encode(&mut encoder) {
                        Ok(()) => println!("Done saving blockchain."),
                        Err(e) => return Err(format!("Failed to write to blockchian: {:?}", e)),
                    }
                },
                Err(e) => return Err(format!("Could not open blockchain for saving: {:?}", e)),
            }
        Ok(())
    }

    fn save_db_state(&mut self) -> Result<(), String> {
        let queue_as_vec: Vec<Sha256dHash> = self.db_state.iter()
            .map(|&hash| hash.clone()).collect();

        let path = beside_path(&self.path_to_chain, "db_state.dat");
        
        match OpenOptions::new()
            .write(true)
            .create(true)
            .open(&path) {
                Ok(file) => {
                    let mut encoder = RawEncoder::new(BufWriter::new(file));
                    match queue_as_vec.consensus_encode(&mut encoder) {
                        Ok(()) => println!("Done saving db state."),
                        Err(e) => return Err(format!("Failed to write db state: {:?}", e)),
                    }
                }
                Err(e) => return Err(format!("Failed to open db_state.dat: {:?}", e)),
            }
        Ok(())
    }
}

fn beside_path(base_path: &String, newfilename: &str) -> String {
    let mut i = base_path.len();
    for c in base_path.chars().rev() {
        if c == '/' {
            break;
        }
        i -= 1;
    }
    let mut path = base_path.chars().take(i).collect::<String>();
    path = path + newfilename;
    path
}

fn load_db_state(path_to_chain: &String) -> Vec<Sha256dHash> {
    let path = beside_path(path_to_chain, "db_state.dat");
    match File::open(&path) {
        Err(e) => {
            println!("Could not open db_state file at {}", path);
            Vec::with_capacity(MAX_BLCKS + 1)
        },
        Ok(file) => {
            let mut decoder = RawDecoder::new(BufReader::new(file));
            match ConsensusDecodable::consensus_decode(&mut decoder) {
                Err(e) => {
                    println!("Could not load db_state at {}: {:?}",
                             path, e);
                    Vec::with_capacity(MAX_BLCKS + 1)
                },
                Ok(queue_as_vec) => queue_as_vec,
            }
        },
    }
    
}

fn load_blockchain(path_to_chain: &String) -> Blockchain {
    match File::open(path_to_chain) {
        Err(e) => {
            println!("Count not open blockchain file at {}", path_to_chain);
            Blockchain::new(Network::Bitcoin)
        },
        Ok(file) => {
            let mut decoder = RawDecoder::new(BufReader::new(file));
            match ConsensusDecodable::consensus_decode(&mut decoder) {
                Err(e) => {
                    println!("Could not load blockchain at {}: {:?}",
                             path_to_chain, e);
                    panic!("Could not load blockchain!");
                    Blockchain::new(Network::Bitcoin)
                },
                Ok(blckchn) => blckchn,
            }
        },
    }
}

