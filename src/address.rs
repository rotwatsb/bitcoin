use secp256k1::key::{PublicKey, SecretKey};
use secp256k1::{Secp256k1, constants};

use rand::os::{OsRng};

use crypto::ripemd160::Ripemd160;
use crypto::sha2::Sha256;
use crypto::digest::Digest;

use rust_base58::ToBase58;

use util;

pub fn new_pri_key(secp: &Secp256k1) -> SecretKey {
    let mut rng: OsRng = OsRng::new().unwrap();
    SecretKey::new(&secp, &mut rng)
}

pub fn pub_from_pri(secp: &Secp256k1, pri_key: &SecretKey) -> PublicKey {
    PublicKey::from_secret_key(secp, pri_key).unwrap()
}

pub fn addr_from_pub(secp: &Secp256k1, pub_key: &PublicKey, for_testnet: bool)
                     -> String {
    let mut data: [u8; constants::PUBLIC_KEY_SIZE] = [0; constants::PUBLIC_KEY_SIZE];
    let pub_key_arrvec = pub_key.serialize_vec(secp, false);
    let iter = pub_key_arrvec.iter();
    
    for (i, byte) in iter.enumerate() {
        data[i] = byte.clone();
    }
    
    let mut sha256 = Sha256::new();
    sha256.input(&data[..]);
    sha256.result(&mut data);
    
    let mut ripemd160 = Ripemd160::new();
    ripemd160.input(&data[..]);
    ripemd160.result(&mut data);
    
    encode_prefix(&mut data, for_testnet);
    
    let checksum = util::checksum(&data[..]);

    data[21] = checksum[0];
    data[22] = checksum[1];
    data[23] = checksum[2];
    data[24] = checksum[3];
            
    data[0..25].to_base58()
}

fn encode_prefix(data: &mut [u8; constants::PUBLIC_KEY_SIZE], for_testnet: bool) {
    let mut temp: u8 = 0;
    let mut temp2: u8 = if for_testnet { 0x6F } else { 0x00 };
    for i in 0..20 {
        temp = data[i];
        data[i] = temp2;
        temp2 = temp;
    }
}
