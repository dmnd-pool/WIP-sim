# ESP32-S3 RTC SAR development model

This is a partial peripheral model. It is not an accepted exact-board model. SAR means successive approximation register. The ADC converts an analog voltage to a digital count.

The static library also holds the separate S3 I2C controller module. See [the I2C scope and evidence](../I2C.md). Its state and C interface are independent of the SAR controller.

The Rust code models two RTC single-conversion channels, their start and done state, pin selection, attenuation, input inversion, clock divider, reset and power cancellation. External channel inputs must be supplied as microvolts. They start disconnected. The C binding has20 initial input properties: `adc1-channel0-uv` through `adc2-channel9-uv`. Each accepts -1 for disconnected, or0 through3300000 microvolts. Defaults remain disconnected. These are independent voltage sources; they do not follow a guest voltage request or establish a physical regulator connection. A reset preserves the external voltage. Compiled tests are required before claiming this QEMU input path works. An unsupported or disconnected conversion does not return a successful sample. The internal ground input is selected only by the guest's analog register command. It produces zero for an ideal ADC. The code does not inject a boot message or bypass firmware instructions.

The C file supplies the QEMU memory regions and virtual timer. C is needed for the existing QEMU object and memory interface. Conversion state and the voltage equation remain in Rust. The backend uses the archived Espressif QEMU CPU implementation. VM migration is explicitly disabled for this incomplete peripheral.

## Exact evidence

- ESP32-S3 TRM version 1.8, archived PDF SHA-256 `4484bf8a69035ec42a731c58c64ada6fbd1f1618c5559409f134d9ea083f444f`. Chapter 39 gives the RTC ADC registers and voltage equation. The PDF remains at `artifacts/files/document/open-esp32-s3-trm/`.
- Espressif QEMU commit `febae182e132e4055529be423a818225ebddaa3a`, archive SHA-256 `5456034eb8a9f348b1f4c8820dc79f55677bddfe0d1c66566b6c42ceb9df66f0`. Its C6 analog-master source supplies a related transaction layout. It does not prove S3 compatibility by itself.
- The actual unchanged S3 factory firmware writes analog command `0x05200769` to `0x6000e000` before starting ADC1. This writes `0x20` to register 7 of block `0x69`. CPU 0 then polls bit 16 at `0x6000880c`. The source IDF analog definitions identify bit 5 as the ADC1 internal-ground selector. The complete diagnostic trace is in `work/simulator/bitaxe/original-probe-014/qemu-trace.log.gz` with its retained raw hash.
- [Espressif IDF v5.5.2 ADC access functions](https://raw.githubusercontent.com/espressif/esp-idf/v5.5.2/components/hal/esp32s3/include/hal/adc_ll.h) define the same start sequence, pin selection and ground configuration. The original firmware reports IDF v5.5.3. The v5.5.2 file is related reference data; it is not a verified v5.5.3 build input.
- [Espressif S3 analog register definitions](https://github.com/espressif/esp-idf/blob/master/components/soc/esp32s3/include/soc/regi2c_saradc.h) corroborate the observed ground command. The retrieved provider text is reference data. It is not an exact archive identity.

## Open work and assumptions

- Timing uses 25 conversion clocks plus the configured sample cycles, divided from an explicit 8 MHz RTC clock. This is a functional approximation. The TRM states the 25-cycle rule for the digital path. RTC conversion latency has not been measured.
- The RTC power-field wording in TRM 1.8 says value 2 forces up and value 3 forces down. The executing firmware writes value 3 before conversion. Related IDF power-control code uses 3 for up and 2 for down. The model currently uses that driver-derived encoding. The exact RTC driver source and this conflict still need verification. No exact target pass may rely on this assumption silently.
- Espressif's published Rust RTC ADC driver also sets `SENS.force_xpd_sar` to 3 for software power on. This corroborates the observed firmware encoding in the RTC register itself. The retained provider reference is `work/simulator/bitaxe/sar_rtc_power_rust-provider.txt`. It is a related reference, not the exact IDF v5.5.3 source used by the factory image.
- Internal analog commands retain register state and readback. Their internal BUSY bit and transaction latency are not implemented; access is synchronous. Initial-code correction and physical ADC offsets are not calibrated. Internal ground uses an ideal zero-offset transfer. A successful guest calibration loop would not establish physical calibration accuracy.
- RTC interrupt wiring, digital conversions, DMA, the ADC2 arbiter and other SENS peripherals are incomplete. Their implementation is required where the selected firmware uses them.
- APB digital ADC registers remain outside this model. Full analog-block semantics are incomplete. Generic stored analog bytes do not constitute PLL, oscillator, power or temperature models.

## Checks

Run from the repository root:

```sh
CARGO_TARGET_DIR=work/simulator/bitaxe/sar-target cargo test --manifest-path simulator/backends/esp32/sar-adc/Cargo.toml --lib --locked --offline
CARGO_TARGET_DIR=work/simulator/bitaxe/sar-target cargo clippy --manifest-path simulator/backends/esp32/sar-adc/Cargo.toml --all-targets --locked --offline -- -D warnings
CARGO_TARGET_DIR=work/simulator/bitaxe/sar-target cargo build --manifest-path simulator/backends/esp32/sar-adc/Cargo.toml --release --locked --offline
```

These component checks passed before the first backend build. Actual backend execution is recorded separately. All exact-board, mining and dashboard gates remain incomplete.

The C ABI check in `abi_check.c` passes with AddressSanitizer and UndefinedBehaviorSanitizer. The first run failed because LeakSanitizer needs a denied host ptrace operation. The second run sets `ASAN_OPTIONS=detect_leaks=0`. This is not a passed leak check. Logs are in `work/simulator/reports/sar-abi-check-00{1,2}.log`.

The extended interface check also passes after adding explicit external
voltages. It checks invalid channel and voltage inputs, a half-scale sample,
input persistence across reset, and disconnection. The command was:

```sh
cc -std=c11 -Wall -Wextra -Werror -fsanitize=address,undefined -fno-omit-frame-pointer simulator/backends/esp32/sar-adc/abi_check.c work/simulator/bitaxe/sar-target/release/libmd_sim_esp32s3_sar.a -lm -ldl -lpthread -o work/simulator/bitaxe/sar-input-abi-check-001
ASAN_OPTIONS=detect_leaks=0 work/simulator/bitaxe/sar-input-abi-check-001
```

Backend 017 has SHA-256
`ce946687f1b97577ba2a6434d08b38a8a6dfeb8cbf072bfbf2545d2e7356cd9e`.
Its 47 compiled qtest transactions pass. They check the conversion deadline,
the half-scale and saturated readings, reset, and a disconnected channel.
Evidence is under `work/simulator/bitaxe/sar-input-qtest-001/`.

Original factory probe 030 uses ADC1/channel1 at an explicit 550000 microvolts.
The firmware completes that conversion and sends Chain Inactive after chip
identification. The identification-only ASIC model refuses that next command.
The guest's missing eFuse calibration still causes a real calibration error.
No calibrated voltage, regulator connection, complete boot or mining pass is claimed.
