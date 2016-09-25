extern crate bitcoin;

mod bitcoind;
mod peerd;
mod util;

use bitcoind::Bitcoind;

fn main() {
    let daemon = Bitcoind::new("127.0.0.1", 8333, "/home/steve/rust/bitcoin/blockchain/bitcoin.dat");
    match daemon.listen() {
        Ok(()) => (),
        Err(e) => println!("{:?}", e),
    }
}





