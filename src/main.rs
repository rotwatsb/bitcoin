extern crate secp256k1;
extern crate rand;
extern crate crypto;

use secp256k1::key::{PublicKey, SecretKey};
use secp256k1::Secp256k1;


mod address;

fn main() {
    let secp: Secp256k1 = Secp256k1::new();
    let pri_key = address::new_pri_key(&secp);
    let pub_key = address::pub_from_pri(&secp, &pri_key);
    println!("{:?}\n{:?}", pri_key, pub_key);

    let mut data: [u8; 32] = pub_key.serialize_vec(&secp, false)
}
