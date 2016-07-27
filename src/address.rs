use secp256k1::key::{PublicKey, SecretKey};
use secp256k1::{Secp256k1};

use rand::os::{OsRng};
use rand::Rng;

use crypto::

pub fn new_pri_key(secp: &Secp256k1) -> SecretKey {
    let mut rng: OsRng = OsRng::new().unwrap();
    SecretKey::new(&secp, &mut rng)
}

pub fn pub_from_pri(secp: &Secp256k1, pri_key: &SecretKey) -> PublicKey {
    PublicKey::from_secret_key(secp, pri_key).unwrap()
}


/*
pub struct Address {
    pri_key: [u8; 32],
}

impl Address {
    pub fn new() -> Address {
        if let Ok(mut osrng) = OsRng::new() {
            let pk = new_private_key(&mut osrng);
            println!("{:?}", pk.to_hex());


            Address {
                pri_key: pk,
            }
        }
        else {
            panic!("Could not create random number generator");
        }
    }
}
*/
/*fn pri_to_pub(pri_key: &[u8; 32]) -> [u8; 32] {
    let bigint_pk = BigInt::from_bytes_be(&pk);
    let curve C256<P256, R256> = C256;
    let g = curve.G();
    let a = bigint_pk * g;
    a.to_bytes_be()[0..32 
}*/
/*
fn new_private_key(rng: &mut Rng) -> [u8; 32] {

    fn is_zero(buf: &[u8; 32]) -> bool {
        for i in 0..32 { if buf[i] != 0 { return false; } }
        true
    }

    fn above_max(buf: &[u8; 32]) -> bool {
        for i in 0..32 {
            if buf[i] > MAX_PRI_KEY[i] { return true }
            else if buf[i] < MAX_PRI_KEY[i] { return false }
        }
        false
    }

    let mut buf: [u8; 32] = [0; 32];
    let mut hasher = Sha256::new();

    while (is_zero(&buf) || above_max(&buf)) {
        rng.fill_bytes(&mut buf);
        hasher.reset();
        hasher.input(&buf);
        hasher.result(&mut buf);
        //println!("{:?}", buf.to_hex());
    }

    buf
}
 */

/*const MAX_PRI_KEY: [u8; 32] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                               0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
                               0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B,
                               0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x40];
 */

