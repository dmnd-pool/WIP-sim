//! Bounded linear decoding of the exact archived PIC14E application.
//! This does not execute instructions, infer bank state, or prove reachability.
//! Opcode reference: Microchip DS40001715D, Table31-3, pages359–360.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::pic16::{Program, APP_HASH, APP_START};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::json;
use std::{fs, path::Path};

fn decode(address: u16, word: u16) -> Option<String> {
    if let Some(name) = match word {
        0 => Some("nop"),
        1 => Some("reset"),
        8 => Some("return"),
        9 => Some("retfie"),
        10 => Some("callw"),
        11 => Some("brw"),
        0x62 => Some("option"),
        0x63 => Some("sleep"),
        0x64 => Some("clrwdt"),
        0x100 => Some("clrw"),
        _ => None,
    } {
        return Some(name.into());
    }
    if word & 0x3fe0 == 0x20 {
        return Some(format!("movlb 0x{:02x}", word & 31));
    }
    if word & 0x3f80 == 0x3180 {
        return Some(format!("movlp 0x{:02x}", word & 127));
    }
    if word & 0x3e00 == 0x3200 {
        let displacement = ((word & 0x1ff) as i16) << 7 >> 7;
        return Some(format!(
            "bra 0x{:04x}",
            (i32::from(address) + 1 + i32::from(displacement)) & 0x7fff
        ));
    }
    if matches!(word & 0x3800, 0x2000 | 0x2800) {
        return Some(format!(
            "{}_low11 0x{:03x}",
            if word & 0x800 == 0 { "call" } else { "goto" },
            word & 0x7ff
        ));
    }
    if matches!(word & 0x3c00, 0x1000 | 0x1400 | 0x1800 | 0x1c00) {
        let name = ["bcf", "bsf", "btfsc", "btfss"][((word - 0x1000) >> 10) as usize];
        return Some(format!(
            "{name} f=0x{:02x}, bit={}",
            word & 127,
            (word >> 7) & 7
        ));
    }
    if matches!(word & 0x3f80, 0x80 | 0x180) {
        return Some(format!(
            "{} f=0x{:02x}",
            if word & 0x100 == 0 { "movwf" } else { "clrf" },
            word & 127
        ));
    }
    if let Some(name) = match word & 0x3f00 {
        0x0200 => Some("subwf"),
        0x0300 => Some("decf"),
        0x0400 => Some("iorwf"),
        0x0500 => Some("andwf"),
        0x0600 => Some("xorwf"),
        0x0700 => Some("addwf"),
        0x0800 => Some("movf"),
        0x0900 => Some("comf"),
        0x0a00 => Some("incf"),
        0x0b00 => Some("decfsz"),
        0x0c00 => Some("rrf"),
        0x0d00 => Some("rlf"),
        0x0e00 => Some("swapf"),
        0x0f00 => Some("incfsz"),
        0x3500 => Some("lslf"),
        0x3600 => Some("lsrf"),
        0x3700 => Some("asrf"),
        0x3b00 => Some("subwfb"),
        0x3d00 => Some("addwfc"),
        _ => None,
    } {
        return Some(format!(
            "{name} f=0x{:02x}, d={}",
            word & 127,
            if word & 128 == 0 { "w" } else { "f" }
        ));
    }
    if let Some(name) = match word & 0x3f00 {
        0x3000 => Some("movlw"),
        0x3400 => Some("retlw"),
        0x3800 => Some("iorlw"),
        0x3900 => Some("andlw"),
        0x3a00 => Some("xorlw"),
        0x3c00 => Some("sublw"),
        0x3e00 => Some("addlw"),
        _ => None,
    } {
        return Some(format!("{name} 0x{:02x}", word & 255));
    }
    if word & 0x3f80 == 0x3100 || word & 0x3f00 == 0x3f00 {
        let offset = ((word & 63) as i16) << 10 >> 10;
        let name = if word & 0x3f80 == 0x3100 {
            "addfsr"
        } else if word & 128 == 0 {
            "moviw"
        } else {
            "movwi"
        };
        return Some(format!("{name} fsr={}, offset={offset}", (word >> 6) & 1));
    }
    if matches!(word & 0x3ff8, 0x10 | 0x18) {
        return Some(format!(
            "{} fsr={}, mode={}",
            if word & 8 == 0 { "moviw" } else { "movwi" },
            (word >> 2) & 1,
            word & 3
        ));
    }
    if (0x65..=0x67).contains(&word) {
        return Some(format!("tris {}", word & 7));
    }
    None
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 3,
        "usage: pic16_static_listing EXACT_APPLICATION NEW_OUTPUT"
    );
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    let input = hash_file(source)?;
    ensure!(
        input.size == 19200
            && input.sha256 == "ec84da07ef98665c6258cc531dc2d74aa8be7018925c041325ca233aec52b46a",
        "PIC application text differs"
    );
    let text = fs::read_to_string(source)?;
    let program = Program::parse(&text).map_err(|e| anyhow::anyhow!("invalid PIC image: {e:?}"))?;
    ensure!(program.sha256() == APP_HASH, "PIC decoded words differ");
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(out)?;
    let mut listing = String::new();
    let mut undecoded = Vec::new();
    for (index, bytes) in program.word_bytes().chunks_exact(2).enumerate() {
        let address = u16::try_from(APP_START + index).context("PIC address bound")?;
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);
        let decoded = decode(address, word);
        if decoded.is_none() {
            undecoded.push(json!({"address":address,"word":word}));
        }
        listing.push_str(&format!(
            "{address:04x} {word:04x} {}\n",
            decoded.as_deref().unwrap_or("UNDECODED")
        ));
    }
    fs::write(out.join("listing.txt"), listing)?;
    save_json(
        &out.join("manifest.json"),
        &json!({"input":input,"word_bytes_sha256":APP_HASH,
        "load_word_address":APP_START,"word_count":3200,"listing":hash_file(&out.join("listing.txt"))?,
        "tool":hash_file(&std::env::current_exe()?)?,"source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("pic16_static_listing.rs")),
        "opcode_reference":"Microchip DS40001715D Table31-3 pages359–360; https://ww1.microchip.com/downloads/en/DeviceDoc/40001715D.pdf",
        "undecoded":undecoded,
        "scope":"Linear static decoding only. Data can decode as instructions. No reachability, bank/page inference, CPU execution, watchdog behavior or hardware acceptance is established. call/goto operands retain unresolved low11 address bits."}),
    )?;
    println!(
        "Saved3200 word decodes; {} undecoded. No CPU execution.",
        undecoded.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn datasheet_examples_and_signed_branch_edges() {
        for (word, expected) in [
            (0x0001, "reset"),
            (0x0064, "clrwdt"),
            (0x3183, "movlp 0x03"),
            (0x0021, "movlb 0x01"),
            (0x110c, "bcf f=0x0c, bit=2"),
            (0x300a, "movlw 0x0a"),
            (0x00a0, "movwf f=0x20"),
            (0x0ba0, "decfsz f=0x20, d=f"),
            (0x33ff, "bra 0x0300"),
            (0x3300, "bra 0x0201"),
            (0x32ff, "bra 0x0400"),
        ] {
            assert_eq!(decode(0x300, word).as_deref(), Some(expected));
        }
        assert!(decode(0x300, 0x0002).is_none());
    }
}
