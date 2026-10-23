//! Read-only A32 disassembly with the captured Capstone5 library.
//! Linear decoding includes data words. It does not establish control flow.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{ffi::CStr, io::Write};

#[repr(C)]
struct Instruction {
    id: u32,
    address: u64,
    size: u16,
    bytes: [u8; 24],
    mnemonic: [libc::c_char; 32],
    operands: [libc::c_char; 160],
    detail: *mut libc::c_void,
}

type Version = unsafe extern "C" fn(*mut i32, *mut i32) -> u32;
type Open = unsafe extern "C" fn(i32, i32, *mut usize) -> i32;
type Close = unsafe extern "C" fn(*mut usize) -> i32;
type OptionFn = unsafe extern "C" fn(usize, i32, usize) -> i32;
type Decode =
    unsafe extern "C" fn(usize, *const u8, usize, u64, usize, *mut *mut Instruction) -> usize;
type Free = unsafe extern "C" fn(*mut Instruction, usize);

struct Capstone {
    library: *mut libc::c_void,
    handle: usize,
    close: Close,
    decode: Decode,
    free: Free,
}
impl Drop for Capstone {
    fn drop(&mut self) {
        unsafe {
            (self.close)(&mut self.handle);
            libc::dlclose(self.library);
        }
    }
}

impl Capstone {
    fn new() -> Result<Self> {
        let library = unsafe {
            libc::dlopen(
                c"/usr/lib/libcapstone.so.5".as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL,
            )
        };
        ensure!(!library.is_null(), "captured Capstone library unavailable");
        let result = (|| {
            let symbol = |name: &CStr| -> Result<*mut libc::c_void> {
                let pointer = unsafe { libc::dlsym(library, name.as_ptr()) };
                ensure!(!pointer.is_null(), "Capstone symbol unavailable: {name:?}");
                Ok(pointer)
            };
            // Signatures and struct layout follow the captured Capstone5 C
            // header. Resolve first, then verify the ABI version before use.
            let version: Version = unsafe { std::mem::transmute(symbol(c"cs_version")?) };
            let mut major = 0;
            let mut minor = 0;
            unsafe {
                version(&mut major, &mut minor);
            }
            ensure!(major == 5 && minor == 0, "Capstone ABI differs");
            let open: Open = unsafe { std::mem::transmute(symbol(c"cs_open")?) };
            let close: Close = unsafe { std::mem::transmute(symbol(c"cs_close")?) };
            let option: OptionFn = unsafe { std::mem::transmute(symbol(c"cs_option")?) };
            let decode: Decode = unsafe { std::mem::transmute(symbol(c"cs_disasm")?) };
            let free: Free = unsafe { std::mem::transmute(symbol(c"cs_free")?) };
            let mut handle = 0;
            ensure!(
                unsafe { open(0, 0, &mut handle) } == 0,
                "Capstone A32 initialization failed"
            );
            if unsafe { option(handle, 5, 3) } != 0 {
                unsafe {
                    close(&mut handle);
                }
                anyhow::bail!("Capstone skip-data setup failed");
            }
            Ok(Self {
                library,
                handle,
                close,
                decode,
                free,
            })
        })();
        if result.is_err() {
            unsafe {
                libc::dlclose(library);
            }
        }
        result
    }

    fn instructions(&self, code: &[u8], address: u64) -> Result<Vec<(u64, String, String)>> {
        let mut pointer = std::ptr::null_mut();
        let count = unsafe {
            (self.decode)(
                self.handle,
                code.as_ptr(),
                code.len(),
                address,
                0,
                &mut pointer,
            )
        };
        ensure!(
            count > 0 && !pointer.is_null(),
            "Capstone produced no instructions"
        );
        let result = (|| {
            ensure!(
                count <= code.len() && count <= 200_000,
                "Capstone instruction count bound"
            );
            let instructions = unsafe { std::slice::from_raw_parts(pointer, count) };
            let text = |field: &[libc::c_char]| -> Result<String> {
                let end = field
                    .iter()
                    .position(|b| *b == 0)
                    .context("unterminated instruction field")?;
                let bytes: Vec<_> = field[..end].iter().map(|b| *b as u8).collect();
                Ok(String::from_utf8(bytes)?)
            };
            let mut result = Vec::with_capacity(count);
            let mut next = address;
            for instruction in instructions {
                ensure!(
                    instruction.address == next && instruction.size == 4,
                    "non-A32 instruction boundary"
                );
                next += u64::from(instruction.size);
                result.push((
                    instruction.address,
                    text(&instruction.mnemonic)?,
                    text(&instruction.operands)?,
                ));
            }
            ensure!(
                next - address == code.len() as u64,
                "incomplete A32 disassembly"
            );
            Ok(result)
        })();
        unsafe {
            (self.free)(pointer, count);
        }
        result
    }
}

pub fn inspect(elf: &[u8], mut output: impl Write) -> Result<Value> {
    ensure!(
        elf.len() >= 52 && elf[..7] == [0x7f, b'E', b'L', b'F', 1, 1, 1] && elf[18..20] == [40, 0],
        "not an ARM ELF32 image"
    );
    let word = |at: usize| -> Result<u32> {
        Ok(u32::from_le_bytes(
            elf.get(at..at + 4)
                .context("ELF word outside file")?
                .try_into()?,
        ))
    };
    let half = |at: usize| -> Result<u16> {
        Ok(u16::from_le_bytes(
            elf.get(at..at + 2)
                .context("ELF halfword outside file")?
                .try_into()?,
        ))
    };
    let offset = usize::try_from(word(32)?)?;
    let size = usize::from(half(46)?);
    let count = usize::from(half(48)?);
    ensure!(
        size == 40 && (1..=256).contains(&count),
        "unsupported ELF section table"
    );
    let end = offset
        .checked_add(size * count)
        .context("ELF section table overflow")?;
    ensure!(end <= elf.len(), "ELF section table outside file");
    let engine = Capstone::new()?;
    let fixed = engine.instructions(&[1, 0, 0xa0, 0xe3, 0x1e, 0xff, 0x2f, 0xe1], 0x1000)?;
    ensure!(
        fixed
            == [
                (0x1000, "mov".into(), "r0, #1".into()),
                (0x1004, "bx".into(), "lr".into())
            ],
        "independent literal A32 vectors differ"
    );
    let mut sections = Vec::new();
    for index in 0..count {
        let at = offset + size * index;
        if word(at + 4)? != 1 || word(at + 8)? & 4 == 0 {
            continue;
        }
        let address = u64::from(word(at + 12)?);
        let start = usize::try_from(word(at + 16)?)?;
        let bytes = usize::try_from(word(at + 20)?)?;
        ensure!(
            bytes > 0 && bytes <= 4 << 20 && bytes % 4 == 0,
            "A32 section size bound"
        );
        let end = start.checked_add(bytes).context("ELF section overflow")?;
        let code = elf.get(start..end).context("ELF section outside file")?;
        let instructions = engine.instructions(code, address)?;
        for (address, mnemonic, operands) in &instructions {
            writeln!(output, "{address:08x} {mnemonic:<10} {operands}")?;
        }
        sections.push(json!({"index":index,"file_offset":start,"address":address,"size":bytes,"instructions":instructions.len()}));
    }
    ensure!(!sections.is_empty(), "executable A32 sections absent");
    Ok(
        json!({"library":"Capstone5.0 ABI, captured installed5.0.9 build","mode":"ARM little-endian A32","literal_vectors_passed":2,"sections":sections,
        "scope":"Static linear disassembly only. Embedded data can decode as instructions. No control-flow or firmware-execution pass is inferred."}),
    )
}
