# EEPROM format

This format was reconstructed by reverse engineering.

## Format description

The format has three sections:

- Device information.
- PT1 and PT2 test results. These are physical and functional tests.
- Sweep results with individual frequency adjustment.

The data is no more than 256 bytes. The full structure fits in a 24C02 I2C EEPROM.
Each section has a CRC, which is a cyclic checksum of the block.
The document names XXTEA as the data encoding algorithm.
Encoding applies to the data sections, except the first two bytes.
Those bytes contain the format version and encryption-key number.

## Format versions

The tool supports reading versions v4 through v6 used on AntMiner devices.
Version v5 adds the Sweep section for individual frequency settings.
Version v6 replaces PIC sensor information with two additional ASIC sensor entries.

## Header: algorithm and key information

- Eeprom Version: 4, 5, or 6. This is the EEPROM data-structure version. Version 4 is used for S19-series boards.
- Algorithm Version: 1 means XXTEA. Version 2 means XOR scrambler.
- Key Version: 1. This identifies the cryptographic keys used to encode EEPROM data.

## Section 1: board and chip identification

The following values are examples from the original document.

- Board SN: HYDTYNGBAAJAI06BE. This is the unique hashboard serial number.
- Chip Die: ED. This is an internal ASIC die revision or type code.
- Chip Marking: S1CT21CB23. This marking is printed on the ASIC package. It helps identify the chip model and production lot.
- Chip Bin: 3. The original describes this as a factory quality category assigned after testing. It says that chips are sorted by efficiency and by the frequencies they can sustain at a given voltage. It claims that a lower bin number means a better chip that needs less voltage. It describes bin 3 as a good standard S19 bin and says that firmware selects voltage from this value.
- Chip Tech: AC. This is an internal manufacturing-process code.
- Board Name: BHB42601. This is the internal hashboard model name.
- PCB Version: 01.00. This is the printed circuit board revision. S19 revisions can have different board routing.
- BOM Version: 00.10. This is the bill-of-materials revision. It identifies the set of resistors, capacitors, integrated circuits, and other parts fitted to the board.

### Sensors and operating parameters

- ASIC sensor Type: 0. This identifies sensors connected to, or integrated in, ASICs. The source says S19 usually reads temperature by a different method, so these integrated-sensor fields are zero. It also names NCT218 as a sensor type.
- ASIC sensor Offsets: 0 0 0 0. These are ASIC address offsets for the temperature sensors. Format v6 provides up to six sensors.
- PIC sensor Type: 142 means LM75A. Value 139 means TMP451. This identifies a main I2C board-temperature sensor read by the PIC microcontroller.
- PIC sensor Mask: 15. This is the address mask for temperature sensors connected through the PIC controller. The bits correspond to addresses 0x48 through 0x4B.
- FT Version: F1V05B2C1. This identifies the factory functional-test software used for the board.
- Factory Job: HYDT20211001003-Y1. HYDT is the assembly-plant code. 2021-10-01 is the production date, 1 October 2021. 003-Y1 identifies the lot or shift.

### Functional-test results: PT1

- PT1 Result: 1, or PASS.
- PT1 Count: 1. This is the number of test cycles.

The original says that this functional test occurs on the assembly line at low frequency, without the cooling system.
A pass means that all ASICs were found on the chain and received preliminary configuration.
The process also loads PIC firmware and identifies temperature sensors.

Use Board Name to recover board topology information.
See the topology examples in the repository's examples directory.

## Section 2: production test 2

PT2 occurs after full board assembly.
It can test one board or a full set of three or four boards.
This section stores the test conditions and results.

- Voltage: 13.60. This is the board supply voltage used during the factory test. The document calls it the reference voltage for this board.
- Frequency: 545. This is the chip frequency in MHz used for the functional test. The document calls it the nominal or maximum reference frequency.
- Nonce Rate: 9997. This measures found nonces during the factory test. It is an internal test metric, not hashrate in TH/s. The source says positive responses can decrease when core frequency increases. A value near 10000 indicates that all chips performed correctly in the test.
- PCB Temp In: 45. This is the board temperature in degrees Celsius at the airflow inlet.
- PCB Temp Out: 65. This is the board temperature in degrees Celsius at the airflow outlet.
- PT2 Result: 1, or PASS.
- PT2 Count: 1. Together, result 1 and count 1 indicate a pass on the first attempt.

## Section 3: individual ASIC frequency settings

This section can be filled during operation or extended pre-sale testing.
It supports power and operating adjustment for each ASIC.

- Sweep Hashrate: 9999. This is the solution-finding metric for the baseline test.
- Sweep Freq base: 545. This is the frequency-encoding base in MHz.
- Sweep Freq step: 5. This is the frequency step in MHz.
- Sweep Data: Array. This is the frequency table.

Each frequency is encoded relative to the base:

`F_i = F_base + F_step * data[i]`

Each chip uses four bits for an offset from 0 to 15.
One byte stores the offsets for two ASICs.
If Sweep Data is present, frequencies are adjusted after the board reaches operating state.
The source describes the table as the maximum frequency for each individual ASIC.

## Translation limits

This is a full English prose translation of the captured document.
Technical field names and example values are retained.
The statements are the community author's explanations. They are not validated Bitmain specifications.
The source's quality-bin ordering, reference-frequency wording, and sensor descriptions need independent checks.
