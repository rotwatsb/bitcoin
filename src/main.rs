extern crate bitcoin;
extern crate postgres;

mod bitcoind;
mod peerd;
mod util;

use bitcoind::Bitcoind;

fn main() {
    let daemon = Bitcoind::new("127.0.0.1", 8333, "/home/steve/rust/bitcoin/blockchain/bitcoin.dat".to_string(), "postgresql://steve:trbmush@localhost/rainbow".to_string());
    match daemon.listen() {
        Ok(()) => (),
        Err(e) => println!("{:?}", e),
    }
}





