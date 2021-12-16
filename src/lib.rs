#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use std::str::FromStr;

use bitreader::BitReader;
use itertools::Itertools;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PacketError {
    #[error("incomplete packet bits")]
    BitsError(#[from] bitreader::BitReaderError),
    #[error("invalid number of arguments `{0}` for operation `{1:?}`")]
    ArgumentError(usize, Operation),
    #[error("invalid operator ID `{0}`")]
    OperatorError(u8),
    #[error("malformed literal value")]
    ValueError,
    #[error(transparent)]
    HexError(#[from] std::num::ParseIntError),
}

enum LengthKind {
    TotalBits(u64),
    PacketCount(usize),
}

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum Operation {
    Sum,
    Product,
    Minimum,
    Maximum,
    GreaterThan,
    LessThan,
    EqualTo,
}

impl TryFrom<u8> for Operation {
    type Error = PacketError;

    fn try_from(op_id: u8) -> Result<Self, Self::Error> {
        match op_id {
            0 => Ok(Self::Sum),
            1 => Ok(Self::Product),
            2 => Ok(Self::Minimum),
            3 => Ok(Self::Maximum),
            // ID 4 is a literal
            5 => Ok(Self::GreaterThan),
            6 => Ok(Self::LessThan),
            7 => Ok(Self::EqualTo),
            _ => Err(PacketError::OperatorError(op_id)),
        }
    }
}

#[derive(Clone, Debug)]
pub enum PacketKind {
    Literal(usize),
    Operator {
        operation: Operation,
        packets: Vec<Packet>,
    },
}

#[derive(Clone, Debug)]
pub struct Packet {
    pub version: u8,
    pub length: u64,
    pub kind: PacketKind,
}

impl Packet {
    /// Evaluates operator packets recursively
    ///
    /// # Errors
    ///
    /// Will return `Err` if any operators have an invalid number of arguments.
    pub fn eval(&self) -> Result<usize, PacketError> {
        Ok(match &self.kind {
            PacketKind::Literal(value) => *value,
            PacketKind::Operator { operation, packets } => {
                let packets: Vec<usize> =
                    packets.iter().map(Self::eval).collect::<Result<_, _>>()?;
                match operation {
                    Operation::Sum => packets.iter().sum(),
                    Operation::Product => packets.iter().product(),
                    Operation::Minimum | Operation::Maximum => *{
                        match operation {
                            Operation::Minimum => packets.iter().min(),
                            Operation::Maximum => packets.iter().max(),
                            _ => unreachable!(),
                        }
                    }
                    .ok_or_else(|| PacketError::ArgumentError(packets.len(), *operation))?,
                    Operation::LessThan | Operation::GreaterThan | Operation::EqualTo => {
                        if let [a, b] = &packets[..] {
                            Ok(match operation {
                                Operation::LessThan => a < b,
                                Operation::GreaterThan => a > b,
                                Operation::EqualTo => a == b,
                                _ => unreachable!(),
                            } as usize)
                        } else {
                            Err(PacketError::ArgumentError(packets.len(), *operation))
                        }?
                    }
                }
            }
        })
    }

    #[must_use]
    /// Returns number of sub-packets contained within this packet, and its packets, recursively
    pub fn packet_count(&self) -> usize {
        self.flat_packets().len() - 1
    }

    #[must_use]
    /// Returns a flattened vec containing Self and its sub-packets
    pub fn flat_packets(&self) -> Vec<&Self> {
        match &self.kind {
            PacketKind::Literal(_) => vec![self],
            PacketKind::Operator {
                operation: _,
                packets,
            } => packets
                .iter()
                .flat_map(Self::flat_packets)
                .chain(std::iter::once(self))
                .collect(),
        }
    }
}

impl<'a> TryFrom<BitReader<'a>> for Packet {
    type Error = PacketError;

    fn try_from(mut bit_reader: BitReader) -> Result<Self, Self::Error> {
        // VVV
        let version = bit_reader.read_u8(3)?;
        // TTT
        let type_id = bit_reader.read_u8(3)?;
        let kind = match type_id {
            4 => {
                let mut bits = Vec::new();
                let mut reading = true;
                // A+, B+, etc...
                while reading {
                    reading = bit_reader.read_bool()?;
                    bits.push(bit_reader.read_u8(4)?);
                }
                let value = bits
                    .into_iter()
                    .map(usize::from)
                    .reduce(|a, b| a << 4 | b)
                    .ok_or(PacketError::ValueError);
                PacketKind::Literal(value?)
            }
            operation => {
                // I
                let length = if bit_reader.read_bool()? {
                    LengthKind::PacketCount(bit_reader.read_u16(11)? as usize)
                } else {
                    LengthKind::TotalBits(bit_reader.read_u64(15)?)
                };
                // A*, B*, etc...
                let mut packets = Vec::new();
                let mut sub_packet_reader = bit_reader.relative_reader();
                while {
                    match length {
                        LengthKind::TotalBits(n_bits) => sub_packet_reader.position() < n_bits,
                        LengthKind::PacketCount(n_packets) => packets.len() < n_packets,
                    }
                } {
                    let reader = sub_packet_reader.relative_reader();
                    let packet = Self::try_from(reader)?;
                    sub_packet_reader.skip(packet.length)?;
                    packets.push(packet);
                }
                bit_reader.skip(sub_packet_reader.position())?;
                let operation = Operation::try_from(operation)?;
                PacketKind::Operator { operation, packets }
            }
        };

        let length = bit_reader.position();
        Ok(Self {
            version,
            length,
            kind,
        })
    }
}

impl FromStr for Packet {
    type Err = PacketError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s
            .chars()
            .chunks(2)
            .into_iter()
            .map(|mut chunk| u8::from_str_radix(&chunk.join(""), 16))
            .collect::<Result<Vec<_>, _>>()?;
        let bit_reader = BitReader::new(&bytes);
        Self::try_from(bit_reader)
    }
}

impl TryFrom<&str> for Packet {
    type Error = PacketError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl TryFrom<String> for Packet {
    type Error = PacketError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.as_str().parse()
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_eval() {
        for (packet, result) in [
            ("C200B40A82", 3),
            ("04005AC33890", 54),
            ("880086C3E88112", 7),
            ("CE00C43D881120", 9),
            ("D8005AC2A8F0", 1),
            ("F600BC2D8F", 0),
            ("9C005AC2F8F0", 0),
            ("9C0141080250320F1802104A08", 1),
            ("6053231004C12DC26D00526BEE728D2C013AC7795ACA756F93B524D8000AAC8FF80B3A7A4016F6802D35C7C94C8AC97AD81D30024C00D1003C80AD050029C00E20240580853401E98C00D50038400D401518C00C7003880376300290023000060D800D09B9D03E7F546930052C016000422234208CC000854778CF0EA7C9C802ACE005FE4EBE1B99EA4C8A2A804D26730E25AA8B23CBDE7C855808057C9C87718DFEED9A008880391520BC280004260C44C8E460086802600087C548430A4401B8C91AE3749CF9CEFF0A8C0041498F180532A9728813A012261367931FF43E9040191F002A539D7A9CEBFCF7B3DE36CA56BC506005EE6393A0ACAA990030B3E29348734BC200D980390960BC723007614C618DC600D4268AD168C0268ED2CB72E09341040181D802B285937A739ACCEFFE9F4B6D30802DC94803D80292B5389DFEB2A440081CE0FCE951005AD800D04BF26B32FC9AFCF8D280592D65B9CE67DCEF20C530E13B7F67F8FB140D200E6673BA45C0086262FBB084F5BF381918017221E402474EF86280333100622FC37844200DC6A8950650005C8273133A300465A7AEC08B00103925392575007E63310592EA747830052801C99C9CB215397F3ACF97CFE41C802DBD004244C67B189E3BC4584E2013C1F91B0BCD60AA1690060360094F6A70B7FC7D34A52CBAE011CB6A17509F8DF61F3B4ED46A683E6BD258100667EA4B1A6211006AD367D600ACBD61FD10CBD61FD129003D9600B4608C931D54700AA6E2932D3CBB45399A49E66E641274AE4040039B8BD2C933137F95A4A76CFBAE122704026E700662200D4358530D4401F8AD0722DCEC3124E92B639CC5AF413300700010D8F30FE1B80021506A33C3F1007A314348DC0002EC4D9CF36280213938F648925BDE134803CB9BD6BF3BFD83C0149E859EA6614A8C", 246225449979)

        ] {
            let packet = Packet::try_from(packet).unwrap();
            assert_eq!(packet.eval().unwrap(), result);
        }
    }
}
