use std::io::{self, Read, Write, BufRead, BufReader};
use byteorder::{ReadBytesExt, WriteBytesExt};
use std::net::{TcpStream, SocketAddr, IpAddr, Ipv4Addr};
use std::time::Duration;

const CONNECT:     u8 = 1;
const CONNACK:     u8 = 2;
const PUBLISH:     u8 = 3;
const PUBACK:      u8 = 4;
const PUBREC:      u8 = 5;
const PUBREL:      u8 = 6;
const PUBCOMP:     u8 = 7;
const SUBSCRIBE:   u8 = 8;
const SUBACK:      u8 = 9;
const UNSUBSCRIBE: u8 = 10;
const UNSUBACK:    u8 = 11;
const PINGREQ:     u8 = 12;
const PINGRESP:    u8 = 13;
const DISCONNECT:  u8 = 14;

struct FixedHeader {
    control_packet_type: u8,

    // Remaining Length is the length of the variable header (10 bytes) plus the length of the Payload. It is encoded in the manner described in section 2.2.3.
    remaining_length: u32,
}

impl FixedHeader {
    pub fn new(control_packet_type: u8, remaining_length: u32) -> FixedHeader {
        FixedHeader {
            control_packet_type,
            remaining_length,
        }
    }

    pub fn read<R: Read>(reader: &mut R) -> Result<FixedHeader, io::Error> {
        let control_packet_type = reader.read_u8()?;
        let remaining_length = {
            let mut cur = 0u32;
            for i in 0.. {
                let byte = reader.read_u8()?;
                cur |= ((byte as u32) & 0x7F) << (7 * i);

                if i >= 4 {
                    return Result::Err(io::Error::new(io::ErrorKind::InvalidData, "malformed reamining length"));
                }
                
                if byte & 0x80 == 0 {
                    break;
                }
            }

            cur
        };

        Result::Ok(FixedHeader::new(control_packet_type, remaining_length))
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8((self.control_packet_type << 4) & 0xF);
        let mut cur_len = self.remaining_length;
        loop {
            let mut byte = (cur_len & 0x7F) as u8;
            cur_len >>= 7;

            if cur_len > 0 {
                byte |= 0x80;
            }

            writer.write_u8(byte)?;

            if cur_len == 0 {
                break;
            }
        }

        Ok(())
    }
}

pub struct ConnectFlags {
    pub user_name: bool,
    pub password: bool,
    pub will_retain: bool,
    pub will_qos: u8,
    pub will_flag: bool,
    pub clean_session: bool,
    // We never use this, but must decode because brokers must verify it's zero per [MQTT-3.1.2-3]
    pub reserved: bool,
}

impl ConnectFlags {
    pub fn new() -> ConnectFlags {
        ConnectFlags {
            user_name: false,
            password: false,
            will_retain: false,
            will_qos: 0,
            will_flag: false,
            clean_session: false,
            reserved: false
        }
    }
}

struct ConnectPacketPayload {
    client_identifier: String,
}

impl ConnectPacketPayload {
    pub fn new(client_identifier: String) -> ConnectPacketPayload {
        ConnectPacketPayload {
            client_identifier,
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_all(self.client_identifier.as_bytes());
        Ok(())
    }
}

struct ConnectPacket {
    fixed_header: FixedHeader,

    // variable header
    protocol_name: [u8; 6],
    protocol_level: u8,
    flags: ConnectFlags,
    keep_alive: u16,

    payload: ConnectPacketPayload,
}

impl ConnectPacket {
    pub fn new(client_identifier: String) -> ConnectPacket {
        ConnectPacket {
            fixed_header: FixedHeader::new(CONNECT, 0),
            protocol_name: [0x0, 0x4, b'M', b'Q', b'T', b'T'],
            protocol_level: 0x04, // spefifying MQTT 3.1.1
            flags: ConnectFlags::new(),
            keep_alive: 0,
            payload: ConnectPacketPayload::new(client_identifier),
        }
    }
    pub fn wirte<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.fixed_header.write(writer);
        writer.write_all(&self.protocol_name);
        Ok(())
    }
}

struct ConnackFlags {
    session_present: bool,
}

impl ConnackFlags {
    fn new(session_present: bool) -> ConnackFlags {
        ConnackFlags { session_present: false }
    }

    fn read<R: Read>(reader: &mut R) -> Result<ConnackFlags, io::Error> {
        let byte = reader.read_u8()?;
        Ok(ConnackFlags {
            session_present: byte & 0x1 == 0x1
        })
    }
}

struct ConnackPacket {
    fixed_header: FixedHeader,
    flags: ConnackFlags,
    ret_code: u8,
}

impl ConnackPacket {
    fn read<R: Read>(reader: &mut R) -> Result<Self, io::Error> {
        let fixed_header = FixedHeader::read(reader)?;
        let flags = ConnackFlags::read(reader)?;
        let ret_code = reader.read_u8()?;

        Ok(ConnackPacket {
            fixed_header,
            flags,
            ret_code,
        })
    }
}

fn main() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1883);
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(30)).expect("failed to connect");
    stream.set_read_timeout(Some(Duration::from_secs(3)));
    stream.set_write_timeout(Some(Duration::from_secs(3)));

    let connect_packet = ConnectPacket::new("test-client".to_string());
    connect_packet.wirte(&mut stream);

    let mut reader = BufReader::new(&stream);
    let result = ConnackPacket::read(&mut reader);

    match &result {
        Ok(connack_packet) => println!("CONNACK ret code: {}", connack_packet.ret_code),
        Err(err) => println!("error: {}", err.to_string()),        
    }
}
