use rustc_serialize::hex::ToHex;
use util;

const MAGIC_TEST: [u8; 4] = [0x0B, 0x11, 0x09, 0x07];
const MAGIC_REAL: [u8; 4] = [0xF9, 0xBE, 0xB4, 0xD9];

const AGENT: [u8; 1] = [0x00];


pub struct Version {
    prot_version: [u8; 4],
    services: [u8; 8],
    timestamp: [u8; 8],
    recipient_info: [u8; 26],
    sender_info: [u8; 26],
    nonce: [u8; 8],
    user_agent: [u8; 1],
    start_height: [u8; 4],
}

impl Version {
    fn new(ip_a: u8, ip_b: u8, ip_c: u8, ip_d: u8) -> Version {
        
        Version {
            prot_version: util::PROTOCOL_VERSION,
            services: util::NODE_NETWORK_SERVICE_00,
            timestamp: util::timestamp()[0..4],
            recipient_info: util::form_net_addr(false, ip_a, ip_b, ip_c, ip_d,
                                                18332)[4..30],
            sender_info: util::form_net_addr(0, 0, 0, 0)[4..30],
            nonce: util::rand_nonce(),
            user_agent: AGENT,
            start_height: [0x55, 0x81, 0x01, 0x00], 
        }
    }
}

