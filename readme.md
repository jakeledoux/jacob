# JACOB &emsp; [![Build Status]][actions] [![Latest Version]][crates.io] [![Documentation]][docs.rs]


[Build Status]: https://github.com/jakeledoux/jacob/actions/workflows/rust.yml/badge.svg
[actions]: https://github.com/jakeledoux/jacob/actions?query=branch%3Amaster
[Latest Version]: https://img.shields.io/crates/v/jacob.svg
[crates.io]: https://crates.io/crates/jacob
[Documentation]: https://img.shields.io/docsrs/jacob/latest
[docs.rs]: https://docs.rs/jacob/latest/jacob/

JACOB is a BITS compiler/decompiler/interpreter for the BITS instruction set.
BITS was introduced as challenge 16 of the 2021's Advent of Code.

# To-do

This is an outline of what I'd like this crate to do. Pull-requests welcome!

- Hex packets
    - [X] decoding
    - [ ] encoding
- Math expressions
    - [ ] decoding
    - [X] encoding
- Packet API
    - [ ] builder API
    - [X] evaluation
    - [X] in-place evaluation (transform into literal)
    - [ ] simplification

# Acronyms

- **JACOB**: **J**acob's **A**wesome **C**ompiler **O**f **B**ITS
- **BITS**: **B**uoyancy **I**nterchange **T**ransmission **S**ystem 
