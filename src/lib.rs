#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use std::str::FromStr;

use bitreader::BitReader;
use bitstream_io::{BigEndian, BitWrite, BitWriter};
use itertools::Itertools;
use thiserror::Error;

const SUM_FUNC: &str = "sum";
const SUM_SYMBOL: &str = "+";
const PRODUCT_FUNC: &str = "product";
const PRODUCT_SYMBOL: &str = "*";
const MINIMUM_FUNC: &str = "min";
const MAXIMUM_FUNC: &str = "max";
const GREATER_THAN_FUNC: &str = "gt";
const GREATER_THAN_SYMBOL: &str = ">";
const LESS_THAN_FUNC: &str = "lt";
const LESS_THAN_SYMBOL: &str = "<";
const EQUAL_TO_FUNC: &str = "eq";
const EQUAL_TO_SYMBOL: &str = "==";

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
    #[error("failed to write bytes")]
    WriteError(#[from] std::io::Error),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Length {
    TotalBits(u64),
    PacketCount(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

impl Operation {
    #[must_use]
    pub const fn as_func_str(&self) -> &'static str {
        match self {
            Operation::Sum => SUM_FUNC,
            Operation::Product => PRODUCT_FUNC,
            Operation::Minimum => MINIMUM_FUNC,
            Operation::Maximum => MAXIMUM_FUNC,
            Operation::GreaterThan => GREATER_THAN_FUNC,
            Operation::LessThan => LESS_THAN_FUNC,
            Operation::EqualTo => EQUAL_TO_FUNC,
        }
    }

    #[must_use]
    pub const fn is_function(&self) -> bool {
        match self {
            Operation::Sum
            | Operation::Product
            | Operation::GreaterThan
            | Operation::LessThan
            | Operation::EqualTo => false,
            Operation::Minimum | Operation::Maximum => true,
        }
    }
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Operation::Sum => SUM_SYMBOL,
                Operation::Product => PRODUCT_SYMBOL,
                Operation::Minimum => MINIMUM_FUNC,
                Operation::Maximum => MAXIMUM_FUNC,
                Operation::GreaterThan => GREATER_THAN_SYMBOL,
                Operation::LessThan => LESS_THAN_SYMBOL,
                Operation::EqualTo => EQUAL_TO_SYMBOL,
            }
        )
    }
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

impl From<&Operation> for u8 {
    fn from(op: &Operation) -> Self {
        match op {
            Operation::Sum => 0,
            Operation::Product => 1,
            Operation::Minimum => 2,
            Operation::Maximum => 3,
            // ID 4 is a literal
            Operation::GreaterThan => 5,
            Operation::LessThan => 6,
            Operation::EqualTo => 7,
        }
    }
}

impl From<Operation> for u8 {
    fn from(op: Operation) -> Self {
        Self::from(&op)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PacketKind {
    Literal(usize),
    Operator {
        length: Length,
        operation: Operation,
        packets: Vec<Packet>,
    },
}

impl PacketKind {
    /// Returns `true` if the packet kind is [`Operator`].
    ///
    /// [`Operator`]: PacketKind::Operator
    #[must_use]
    pub const fn is_operator(&self) -> bool {
        matches!(self, Self::Operator { .. })
    }

    /// Returns `true` if the packet kind is [`Literal`].
    ///
    /// [`Literal`]: PacketKind::Literal
    #[must_use]
    pub const fn is_literal(&self) -> bool {
        matches!(self, Self::Literal(..))
    }
}

impl From<&PacketKind> for u8 {
    fn from(kind: &PacketKind) -> Self {
        match kind {
            PacketKind::Literal { .. } => 4,
            PacketKind::Operator { operation, .. } => Self::from(operation),
        }
    }
}

impl From<PacketKind> for u8 {
    fn from(kind: PacketKind) -> Self {
        Self::from(&kind)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub version: u8,
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
            PacketKind::Operator {
                operation, packets, ..
            } => {
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

    pub fn to_literal(&self) -> Result<Self, PacketError> {
        Ok(Self {
            version: self.version,
            kind: PacketKind::Literal(self.eval()?),
        })
    }

    /// Returns packet as byte array
    ///
    /// # Errors
    ///
    /// Will return `Err` if serialization fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PacketError> {
        let mut bytes: Vec<u8> = Vec::new();
        let mut writer = BitWriter::endian(&mut bytes, bitstream_io::BigEndian);
        self.write_bits(&mut writer)?;

        // Add packing zeros
        writer.byte_align()?;

        Ok(bytes)
    }

    /// Returns hexadecimal representation of packet
    ///
    /// # Errors
    ///
    /// Will return `Err` if serialization fails.
    pub fn to_hex(&self) -> Result<String, PacketError> {
        Ok(hex_from_bytes(&self.to_bytes()?))
    }

    /// Serializes packet as bits into writer
    ///
    /// # Errors
    ///
    /// Will return `Err` if write fails.
    pub fn write_bits(
        &self,
        writer: &mut BitWriter<&mut Vec<u8>, BigEndian>,
    ) -> Result<(), PacketError> {
        // VVV
        writer.write(3, self.version)?;
        // TTT
        writer.write::<u8>(3, u8::from(&self.kind))?;

        match &self.kind {
            PacketKind::Literal(value) => {
                // Convert value to binbary string representation
                let value = format!("{:b}", value);
                // Zero-pad bits to multiple of 3
                let mut value = std::iter::repeat('0')
                    .take((4 - value.len() % 4) % 4)
                    .chain(value.chars())
                    .map(|c| if c == '0' { 0 } else { 1 })
                    .collect_vec();
                value.reverse();

                while !value.is_empty() {
                    writer.write(1, if value.len() > 4 { 1 } else { 0 })?;
                    let bits: Vec<u8> = std::iter::repeat_with(|| value.pop())
                        .take(4)
                        .collect::<Option<Vec<_>>>()
                        .expect("value is a multiple of 4");
                    let n: u8 = bits[0] << 3 | bits[1] << 2 | bits[2] << 1 | bits[3];
                    writer.write(4, n)?;
                }
            }
            PacketKind::Operator {
                packets, length, ..
            } => {
                match length {
                    Length::TotalBits(bits) => {
                        // I
                        writer.write(1, 0)?;
                        // L
                        writer.write(15, *bits)?;
                    }
                    Length::PacketCount(packets) => {
                        // I
                        writer.write(1, 1)?;
                        // L
                        writer.write(11, *packets as u16)?;
                    }
                }

                for packet in packets {
                    packet.write_bits(writer)?;
                }
            }
        };

        Ok(())
    }

    /// Returns number of sub-packets contained within this packet, and its packets, recursively
    #[must_use]
    pub fn packet_count(&self) -> usize {
        self.flat_packets().len() - 1
    }

    /// Returns a flattened vec containing Self and its sub-packets
    #[must_use]
    pub fn flat_packets(&self) -> Vec<&Self> {
        match &self.kind {
            PacketKind::Literal(_) => vec![self],
            PacketKind::Operator { packets, .. } => packets
                .iter()
                .flat_map(Self::flat_packets)
                .chain(std::iter::once(self))
                .collect(),
        }
    }

    /// Renders to mathematical expression representation
    ///
    /// # Errors
    ///
    /// Will return `Err` if packet does not evaluate properly
    pub fn to_expression(&self) -> Result<String, PacketError> {
        match &self.kind {
            PacketKind::Literal(value) => Ok(value.to_string()),
            PacketKind::Operator {
                operation, packets, ..
            } => {
                let mut packet_expressions = packets
                    .iter()
                    .map(Self::to_expression)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .zip(packets.iter())
                    .map(|(expr, packet)| match packet.kind {
                        PacketKind::Operator { operation, .. } if !operation.is_function() => {
                            format!("({})", expr)
                        }
                        _ => expr,
                    });
                if operation.is_function() {
                    Ok(format!(
                        "{func}({args})",
                        func = operation.to_string(),
                        args = packet_expressions.join(", ")
                    ))
                } else {
                    let args = packet_expressions.collect_vec();
                    Ok(match args.len() {
                        1 => {
                            format!(
                                "{func}({args})",
                                func = operation.as_func_str(),
                                args = args.join(", ")
                            )
                        }
                        _ => args.join(&format!(" {} ", operation.to_string())),
                    })
                }
            }
        }
    }
}

impl<'a> TryFrom<&mut BitReader<'a>> for Packet {
    type Error = PacketError;

    fn try_from(bit_reader: &mut BitReader) -> Result<Self, Self::Error> {
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
                    Length::PacketCount(bit_reader.read_u16(11)?)
                } else {
                    Length::TotalBits(bit_reader.read_u64(15)?)
                };
                // A*, B*, etc...
                let mut packets = Vec::new();
                let mut sub_packet_reader = bit_reader.relative_reader();
                while {
                    match length {
                        Length::TotalBits(n_bits) => sub_packet_reader.position() < n_bits,
                        Length::PacketCount(n_packets) => packets.len() < n_packets as usize,
                    }
                } {
                    let mut reader = sub_packet_reader.relative_reader();
                    let packet = Self::try_from(&mut reader)?;
                    sub_packet_reader.skip(reader.position())?;
                    packets.push(packet);
                }
                bit_reader.skip(sub_packet_reader.position())?;
                let operation = Operation::try_from(operation)?;
                PacketKind::Operator {
                    operation,
                    packets,
                    length,
                }
            }
        };

        Ok(Self { version, kind })
    }
}

impl FromStr for Packet {
    type Err = PacketError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = bytes_from_hex(s)?;
        let mut bit_reader = BitReader::new(&bytes);
        Self::try_from(&mut bit_reader)
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

/// Converts a hexadecimal string into a byte array
///
/// # Errors
///
/// Will return `Err` if string is not valid hexadecimal.
pub fn bytes_from_hex(hex: &str) -> Result<Vec<u8>, PacketError> {
    Ok(hex
        .chars()
        .chunks(2)
        .into_iter()
        .map(|mut chunk| u8::from_str_radix(&chunk.join(""), 16))
        .collect::<Result<Vec<_>, _>>()?)
}

/// Converts a byte array into a hexadecimal string
#[must_use]
pub fn hex_from_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{:02X}", byte))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use crate::{bytes_from_hex, hex_from_bytes, Packet, PacketKind};
    static TEST_CASES: &[TestCase] = &[
        TestCase {
            hex: "D2FE28",
            eval: 2021,
            expr: "2021",
        },
        TestCase {
            hex: "C200B40A82",
            eval: 3,
            expr: "1 + 2",
        },
        TestCase {
            hex: "04005AC33890",
            eval: 54,
            expr: "6 * 9",
        },
        TestCase {
            hex: "880086C3E88112",
            eval: 7,
            expr: "min(7, 8, 9)",
        },
        TestCase {
            hex: "CE00C43D881120",
            eval: 9,
            expr: "max(7, 8, 9)",
        },
        TestCase {
            hex: "D8005AC2A8F0",
            eval: 1,
            expr: "5 < 15",
        },
        TestCase {
            hex: "F600BC2D8F",
            eval: 0,
            expr: "5 > 15",
        },
        TestCase {
            hex: "9C005AC2F8F0",
            eval: 0,
            expr: "5 == 15",
        },
        TestCase {
            hex: "9C0141080250320F1802104A08",
            eval: 1,
            expr: "(1 + 3) == (2 * 2)",
        },
        TestCase {
            hex: "6053231004C12DC26D00526BEE728D2C013AC7795ACA756F93B524D8000AAC8FF80B3A7A4016F6802D35C7C94C8AC97AD81D30024C00D1003C80AD050029C00E20240580853401E98C00D50038400D401518C00C7003880376300290023000060D800D09B9D03E7F546930052C016000422234208CC000854778CF0EA7C9C802ACE005FE4EBE1B99EA4C8A2A804D26730E25AA8B23CBDE7C855808057C9C87718DFEED9A008880391520BC280004260C44C8E460086802600087C548430A4401B8C91AE3749CF9CEFF0A8C0041498F180532A9728813A012261367931FF43E9040191F002A539D7A9CEBFCF7B3DE36CA56BC506005EE6393A0ACAA990030B3E29348734BC200D980390960BC723007614C618DC600D4268AD168C0268ED2CB72E09341040181D802B285937A739ACCEFFE9F4B6D30802DC94803D80292B5389DFEB2A440081CE0FCE951005AD800D04BF26B32FC9AFCF8D280592D65B9CE67DCEF20C530E13B7F67F8FB140D200E6673BA45C0086262FBB084F5BF381918017221E402474EF86280333100622FC37844200DC6A8950650005C8273133A300465A7AEC08B00103925392575007E63310592EA747830052801C99C9CB215397F3ACF97CFE41C802DBD004244C67B189E3BC4584E2013C1F91B0BCD60AA1690060360094F6A70B7FC7D34A52CBAE011CB6A17509F8DF61F3B4ED46A683E6BD258100667EA4B1A6211006AD367D600ACBD61FD10CBD61FD129003D9600B4608C931D54700AA6E2932D3CBB45399A49E66E641274AE4040039B8BD2C933137F95A4A76CFBAE122704026E700662200D4358530D4401F8AD0722DCEC3124E92B639CC5AF413300700010D8F30FE1B80021506A33C3F1007A314348DC0002EC4D9CF36280213938F648925BDE134803CB9BD6BF3BFD83C0149E859EA6614A8C",
            eval: 246_225_449_979,
            expr: "(1732 * (2814 < 77)) + max(14, 5579613, 222253) + (8128 + 215) + ((2767 < 1170) * 190083) + (product((product((sum((product(min(max((product(max(min((product(min((sum(min((product(max((product((product(min((sum(max(45889))))))))))))))))))))))))))))))) + 64077 + (((8 + 4 + 12) > (14 + 12 + 7)) * 244) + ((13795 == 2521) * 24) + min(55, 7, 1624, 7641219164) + (51766673277 * ((10 + 2 + 5) < (3 + 9 + 14))) + (((10 + 4 + 5) < (12 + 13 + 13)) * 869064586) + max(51) + (89 * 72 * 208 * 22 * 183) + 9429241 + ((3295 == 3295) * 15637965) + 284106 + max(574274, 90) + (242 * 168 * 171) + ((4 * 2 * 14) + (5 * 12 * 13) + (4 * 10 * 11)) + (14 * 107 * 112 * 161) + ((69 > 2990) * 177438679) + 1721 + (1024 * (1367 > 916122)) + (195 * 213) + ((31803 < 31803) * 243) + min(1643, 54927350796, 142, 3622435068, 1) + (52648 * (555874 < 15135494)) + (product(17)) + (3555 * ((11 + 6 + 4) > (13 + 9 + 3))) + min(2) + 2103 + (6532356 * (42 < 42)) + min(35088, 729, 15) + ((799377 > 51182) * 245) + 3984 + ((22 < 3900935624) * 4) + (3 + 354 + 2693 + 5) + ((3929042919 > 170) * 107) + max(434298, 989105, 871763, 161) + 44587 + (3924 + 13 + 8) + (183 * (7671716 > 7671716)) + ((12 > 12) * 2266) + max(2841, 25502, 10, 37935, 2868) + 214416 + (11 + 105 + 2111 + 22585712350 + 23) + (((6 + 5 + 6) == (10 + 3 + 11)) * 854057) + 165570701122 + ((15 + 7 + 6) * (3 + 12 + 7) * (12 + 6 + 13)) + (sum(9)) + 3309 + min(12786984, 179081) + (3132045308 * (57455 == 590931))",
        },
    ];

    struct TestCase {
        hex: &'static str,
        eval: usize,
        expr: &'static str,
    }

    #[test]
    fn test_hex_from_bytes_and_bytes_from_hex() {
        for case in TEST_CASES {
            let bytes = bytes_from_hex(case.hex).unwrap();
            let hex = hex_from_bytes(&bytes);
            assert_eq!(hex, case.hex);
            assert_eq!(bytes_from_hex(&hex).unwrap(), bytes);
        }
    }

    #[test]
    fn test_to_bytes_and_to_hex() {
        for case in TEST_CASES {
            let packet = Packet::try_from(case.hex).unwrap();
            let encoded_hex = packet.to_hex().unwrap();
            assert_eq!(encoded_hex, case.hex);
        }
    }

    #[test]
    fn test_eval() {
        for case in TEST_CASES {
            let packet = Packet::try_from(case.hex).unwrap();
            assert_eq!(packet.eval().unwrap(), case.eval);
        }
    }

    #[test]
    fn test_to_literal() {
        for case in TEST_CASES {
            let packet = Packet::try_from(case.hex).unwrap();
            let expected_packet = Packet {
                version: packet.version,
                kind: PacketKind::Literal(case.eval),
            };
            assert_eq!(packet.to_literal().unwrap(), expected_packet);
        }
    }

    #[test]
    fn test_to_expression() {
        for case in TEST_CASES {
            let packet = Packet::try_from(case.hex).unwrap();
            assert_eq!(packet.to_expression().unwrap(), case.expr);
        }
    }
}
