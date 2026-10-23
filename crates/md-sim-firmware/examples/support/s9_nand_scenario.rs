//! Native NAND test commands and evidence assertions. No device emulation here.
use anyhow::{ensure, Result};
use md_sim_firmware::prepare::hash_bytes;

pub const PATTERN_BYTES: usize = 65 * 64;
pub const PHASES: [&str; 3] = ["write", "read-erase", "erased-readback"];

const HEADER: &str =
    "(\nset -eu\ngrep -Fx 'mtd5: 00080000 00020000 \"miner_cfg\"' /proc/mtd\n[ -c /dev/mtd5 ]\n";
const FOOTER: &str = r#"attempts=0
until logread | grep -F 'procd: - init complete -'; do
    attempts=$((attempts + 1))
    [ "$attempts" -le 120 ]
    sleep 1
done
sync
)
"#;
const MARKERS: [&[&str]; 3] = [
    &["MD_SIM_NATIVE_NAND_WRITE_READBACK_VERIFIED"],
    &[
        "MD_SIM_NATIVE_NAND_RESTART_READBACK_VERIFIED",
        "MD_SIM_NATIVE_NAND_ERASE_READBACK_VERIFIED",
    ],
    &["MD_SIM_NATIVE_NAND_ERASE_RESTART_READBACK_VERIFIED"],
];

pub fn pattern(token: &str) -> Result<Vec<u8>> {
    ensure!(
        token.len() == 64
            && token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "NAND pattern requires one lower-case 32-byte hexadecimal token"
    );
    Ok(format!("{token}\n").repeat(64).into_bytes())
}

fn read_erased() -> String {
    format!("dd if=/dev/mtd5 of=/tmp/nand-erased bs=1 count={PATTERN_BYTES}\n[ \"$(sha256sum /tmp/nand-erased | cut -d ' ' -f 1)\" = {} ]\n",
        hash_bytes(&vec![0xff; PATTERN_BYTES]))
}

pub fn storage_commands(phase: usize, token: &str) -> Result<String> {
    storage_commands_with_sync(phase, token, true)
}

pub fn storage_commands_with_sync(phase: usize, token: &str, final_sync: bool) -> Result<String> {
    let bytes = pattern(token)?;
    ensure!(phase < PHASES.len(), "unknown NAND phase");
    let mut script = HEADER.to_owned();
    if phase == 0 || phase == 2 {
        script.push_str(&read_erased());
    }
    if phase < 2 {
        script.push_str(&format!(
            "(for block in $(seq 1 64); do printf '%s\\n' '{token}'; done) > /tmp/nand-pattern\n"
        ));
        if phase == 0 {
            script.push_str("mtd write /tmp/nand-pattern miner_cfg\n");
        }
        script.push_str(&format!("dd if=/dev/mtd5 of=/tmp/nand-readback bs=1 count={PATTERN_BYTES}\ncmp /tmp/nand-pattern /tmp/nand-readback\n[ \"$(sha256sum /tmp/nand-readback | cut -d ' ' -f 1)\" = {} ]\nsha256sum /tmp/nand-pattern /tmp/nand-readback\n",
            hash_bytes(&bytes)));
    }
    script.push_str(&format!("printf '\\n{}\\n'\n", MARKERS[phase][0]));
    if phase == 1 {
        script.push_str("mtd erase miner_cfg\n");
        script.push_str(&read_erased());
        script.push_str(&format!("printf '\\n{}\\n'\n", MARKERS[phase][1]));
    }
    if final_sync {
        script.push_str(FOOTER);
    } else {
        script.push_str(&FOOTER.replace("\nsync\n", "\n"));
    }
    Ok(script)
}

pub fn verify_native(guest: &str, phase: usize) -> Result<()> {
    ensure!(phase < PHASES.len(), "unknown NAND phase");
    let mut last = 0;
    for (index, markers) in MARKERS.iter().enumerate() {
        for marker in *markers {
            let needle = format!("\n{marker}\r\n");
            let found: Vec<_> = guest.match_indices(&needle).map(|(at, _)| at).collect();
            ensure!(
                found.len() == usize::from(index == phase),
                "native NAND phase marker absent, repeated or wrong: {marker}"
            );
            if let Some(at) = found.first() {
                ensure!(*at >= last, "native NAND phase order differs");
                last = *at;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_evidence_requires_the_exact_phase_once_and_in_order() {
        let read = "\nMD_SIM_NATIVE_NAND_RESTART_READBACK_VERIFIED\r\n";
        let erase = "\nMD_SIM_NATIVE_NAND_ERASE_READBACK_VERIFIED\r\n";
        verify_native(&format!("{read}{erase}"), 1).unwrap();
        for text in [
            read.to_owned(),
            format!("{erase}{read}"),
            format!("{read}{erase}{erase}"),
            format!("prefix{read}suffix"),
        ] {
            assert!(verify_native(&text, 1).is_err());
        }
        assert!(verify_native(&format!("{read}{erase}"), 0).is_err());
        assert!(verify_native("", 3).is_err());
    }

    #[test]
    fn pattern_input_is_not_a_shell_expression() {
        for input in [
            "",
            "$(touch /work/file)",
            &"A".repeat(64),
            &"0".repeat(63),
            &"0".repeat(65),
        ] {
            assert!(pattern(input).is_err());
        }
        let data = pattern(&"0".repeat(64)).unwrap();
        assert_eq!(data.len(), PATTERN_BYTES);
        assert!(data.len() > 2 * 2048);
        assert_eq!(data.iter().filter(|b| **b == b'\n').count(), 64);
    }
}
