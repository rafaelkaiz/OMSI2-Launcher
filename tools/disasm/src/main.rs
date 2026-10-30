//! Usage: disasm <exe> <start-va-hex> <length-or-end-va-hex> [--end]
//! Prints the disassembly of that range. Never writes anything.

use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, IntelFormatter};

struct Section {
    va: u32,
    raw: u32,
    size: u32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: disasm <exe> <start-va> <length> [--end]");
        return;
    }
    let data = std::fs::read(&args[1]).expect("cannot read the program");
    let start = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).expect("bad start");
    let second = u32::from_str_radix(args[3].trim_start_matches("0x"), 16).expect("bad length");
    let end = if args.get(4).is_some_and(|a| a == "--end") { second } else { start + second };

    let pe = u32::from_le_bytes(data[0x3c..0x40].try_into().unwrap()) as usize;
    let nsec = u16::from_le_bytes(data[pe + 6..pe + 8].try_into().unwrap()) as usize;
    let opt = pe + 24;
    let base = u32::from_le_bytes(data[opt + 28..opt + 32].try_into().unwrap());
    let so = opt + u16::from_le_bytes(data[pe + 20..pe + 22].try_into().unwrap()) as usize;
    let secs: Vec<Section> = (0..nsec)
        .map(|i| {
            let s = so + i * 40;
            let u = |o: usize| u32::from_le_bytes(data[s + o..s + o + 4].try_into().unwrap());
            Section { va: u(12), raw: u(20), size: u(16) }
        })
        .collect();
    let off = |va: u32| -> Option<usize> {
        let r = va.checked_sub(base)?;
        secs.iter().find(|s| r >= s.va && r < s.va + s.size).map(|s| (r - s.va + s.raw) as usize)
    };
    let from = off(start).expect("start not in a section");
    let bytes = &data[from..from + (end - start) as usize];

    let mut decoder = Decoder::with_ip(32, bytes, start as u64, DecoderOptions::NONE);
    let mut fmt = IntelFormatter::new();
    fmt.options_mut().set_hex_prefix("0x");
    fmt.options_mut().set_hex_suffix("");
    let mut ins = Instruction::default();
    let mut text = String::new();
    while decoder.can_decode() {
        decoder.decode_out(&mut ins);
        text.clear();
        fmt.format(&ins, &mut text);
        let o = (ins.ip() as u32 - start) as usize;
        let hex: String = bytes[o..o + ins.len()].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
        println!("{:08X}  {:<24} {}", ins.ip(), hex, text);
    }
}
