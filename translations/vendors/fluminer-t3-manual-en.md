# FLUMINER T3 operation manual — English technical companion

Original: `vendors-fluminer-t3-manual-cn`, seven pages, February 2026. Publisher: Flu Electronic Technology (Hong Kong) Co., Limited. The source is Chinese even though the English download page labels it “Operation Manual.” This companion translates the technical instructions and specifications. The introductory sales prose is summarized. Text embedded only in pictures is not covered. AI translation; no human review.

The manufacturer page and manual differ. The page gives **105 TH/s and 1650 W**. This manual gives **115 TH/s and 1700 W**. Keep both profiles. Do not infer that firmware 1.15 changes one profile into the other.

## 1. Product

The manufacturer describes T3 as an air-cooled Bitcoin SHA-256 miner for homes and small server installations. Its introduction claims custom ASIC architecture, opposing turbine airflow, magnetic-bearing fans and a core temperature near 65°C under full load. These are manufacturer claims, not measurements made for this catalogue.

The manual lists:

| Item | Value |
|---|---|
| Hash rate | 115 TH/s ±5% |
| Coins named by the source | BTC, BCH, NMC, FB, SYS, ELA |
| Power | 1700 W ±10% |
| Cooling | Air |
| Noise | About 50 dB with ambient noise below 20 dB; another paragraph gives 50–60 dB |
| Dimensions | 447 × 350 × 131 mm; the introduction instead says 450 × 140 × 300 mm |
| Network | 10/100 Ethernet and “WiFi 5.0” in the specification list |
| Included items | One T3, one power cable and one server mounting bracket |

The stated main features are low noise, household power use and simple startup.

## 2. Prepare the installation

Inspect the case and connectors for damage. Check for abnormal sounds, loose screws or loose metal parts. If you find damage or loose parts, do not apply power. Contact the seller or manufacturer service team.

Keep the inlet and outlet clear in a well-ventilated place. Use an earthed socket. Do not share it with another high-power appliance. Keep children away from the device and cable.

The stated environment is 0–35°C, 10–90% relative humidity without condensation, and altitude at or below 2000 m. Supply AC 220–240 V, 50–60 Hz; stated maximum input current is 16 A.

Provide a network that permits mining. This section lists 10/100 Ethernet and both 2.4 GHz and 5 GHz Wi-Fi. A later precaution says to use a wired connection with a router that has no 2.4 GHz band. This inconsistency remains unresolved; it does not prove that every hardware revision supports both bands.

Place the unit on a stable surface. Use a router or switch without PoE. Do not stack or cover devices. Keep dust and foreign material out. The source says that unofficial hardware changes or firmware can void its warranty. It also says not to remove screws, fasteners, labels or stickers. The case becomes hot during use; avoid contact while it runs.

## 3. Setup

Connect the power cable, connect Ethernet to the router, and install the supplied Wi-Fi antenna. A green indicator shows normal operation.

Find the device's IP address with the manufacturer's batch tool or a LAN discovery tool. Open that address in a browser on the same network segment.

Without Ethernet, the source says that the device exposes an open access point named `flu_T3_xxxx`. Connect to it, open `10.42.0.1`, enter the intended Wi-Fi name and password, then restart the device. The original has a spelling error, “wife,” for Wi-Fi.

In pool settings, change the pool and worker name as required.

The manual then repeats section number 3.4 for operating modes:

- Quiet mode favors lower noise and energy efficiency.
- Normal mode balances noise and hash rate.
- High-performance mode gives maximum hash rate with more power and heat. The source requires adequate airflow to avoid instability.

## 4. Routine use

To start, supply power and connect Ethernet or Wi-Fi.

To stop, disconnect Ethernet or Wi-Fi first. Let the device run for three minutes, then remove power.

Check the screen or web interface for hash rate, status and power. Check pool statistics for hash rate and rejected work. The original status list uses the Chinese word for “stability”; it is not silently changed here into a temperature measurement.

## 5. Maintenance

Inspect and clean inlet and outlet dust every two weeks with a dry soft brush or blower.

Read firmware notices on the official site. Perform updates through the web interface. If the fan becomes noisier, contact service about replacement.

## 6. Problems

For a network failure, check the network connections or restart the router.

For excess temperature, improve ventilation and clean the air openings. The source says to replace heatsink thermal material after more than one year of use. It gives no disassembly method here.

If the pool does not show the device, check the pool account and wait about ten minutes after startup.

## 7. Support

The source states a 360-day repair warranty for damage that is not caused by the user. Its support contacts are the public `fluminergroup` Telegram group and `fluminer2024@gmail.com`. This catalogue did not contact them.

## Other saved Fluminer files

`vendors-fluminer-t3-upgrade-notice` already contains Chinese and English instructions. It describes the older `T3_1.12_0226_log.zip` and a cache-clearing step. It is not the release note for 1.15.

`vendors-fluminer-groupcontrol-1-3-0` is an encrypted ZIP. Its Chinese directory label `运维客户版` means “customer operations and maintenance edition”; `密码_fluminer` means “password_fluminer.” The archive was saved byte-for-byte. No program was run.
