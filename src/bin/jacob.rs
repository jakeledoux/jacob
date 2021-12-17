use std::str::FromStr;

use clap::{ArgEnum, Parser};
use jacob::Packet;

#[derive(Parser, ArgEnum, Clone, Copy, Debug)]
enum InFormat {
    #[clap(name = "hex")]
    Hex,
    #[clap(name = "expr")]
    Expression,
    // TODO:
    // #[clap(name = "bin")]
    // Binary
}

#[derive(Parser, ArgEnum, Clone, Copy)]
enum OutFormat {
    #[clap(name = "hex")]
    Hex,
    #[clap(name = "expr")]
    Expression,
    #[clap(name = "eval")]
    Eval,
    // TODO:
    // #[clap(name = "bin")]
    // Binary
}

/// Simple program to greet a person
#[derive(Parser)]
#[clap(about, version, author)]
struct Args {
    #[clap(arg_enum, short, long, default_value = "hex")]
    in_format: InFormat,

    #[clap(arg_enum, short, long, default_value = "eval")]
    out_format: OutFormat,

    #[clap(required = true)]
    inputs: Vec<String>,
}

fn main() {
    let args = Args::parse();

    // TODO: Read from stdin/pipe if args.inputs is empty
    for packet_str in args.inputs {
        if let Ok(packet) = match args.in_format {
            InFormat::Hex => Packet::from_str(&packet_str),
            InFormat::Expression => {
                eprintln!("Expression parsing has not yet been implemented.");
                return;
            }
        } {
            match match args.out_format {
                OutFormat::Hex => packet.to_hex(),
                OutFormat::Expression => packet.to_expression(),
                OutFormat::Eval => packet.eval().map(|n| n.to_string()),
            } {
                Ok(result) => {
                    println!("{}", result);
                }
                Err(e) => {
                    eprintln!("Failed to evaluate packet. Full error:\n{}", e);
                }
            }
        } else {
            eprintln!("Failed to parse packet with format: `{:?}`", args.in_format);
        }
    }
}
