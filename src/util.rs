use std::sync::mpsc::{Sender};
use std::net::Ipv4Addr;

use bitcoin::network::address::Address;
use bitcoin::blockdata::block::{LoneBlockHeader, Block};
use bitcoin::blockdata::transaction::{Transaction, TxOut};
use bitcoin::network::message_blockdata::Inventory;
use bitcoin::util::hash::{Sha256dHash};
use bitcoin::util::base58::ToBase58;

pub enum ThreadResponse {
    Addresses(Vec<(u32, Address)>),
    Headers(String, Vec<LoneBlockHeader>),
    Inv(String, Vec<Inventory>),
    Block(Block),
    Tx(Transaction),
    CloseThread((String, Sender<()>)),
}

pub fn ipv4_to_ipv4addr(ip: &str) -> Ipv4Addr {
    let ip_vec = ip.to_string().split('.')
        .map(|slice| slice.trim().parse::<u8>().unwrap())
        .collect::<Vec<u8>>();
    Ipv4Addr::new(ip_vec[0], ip_vec[1], ip_vec[2], ip_vec[3])
}

pub fn string_of_address(address: &Address) -> String {
    (address.address[6] / 256).to_string() + "." +
        &((address.address[6] % 256).to_string()) + "." +
        &((address.address[7] / 256).to_string()) + "." +
        &((address.address[7] % 256).to_string())
}

fn addr_from_hash(hash: &[u8]) -> String {
    let mut v = vec![0];
    v.extend_from_slice(hash);
    let hashed_hash = Sha256dHash::from_data(&v[..]);
    v.extend_from_slice(&hashed_hash[..4]);
    v.to_base58()
}

pub fn addr_from_output(output: &TxOut) -> Option<String> {
    let v = output.script_pubkey.clone().into_vec();
    let mut j = 0;
    let l = v.len();
    while j < l && v[j] != 20 { j += 1 }
    if v.len() >= j + 21 {
        Some(addr_from_hash(&v[j + 1..j + 21]))
    } else { None }
}

