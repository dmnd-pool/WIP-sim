# WhatsMiner M31S integrated miner operation guide V1.1

Source: `vendors-wm-legacy-docs-operation-guide-m31s-v1-1-pdf`, 30 pages. Publisher: Shenzhen MicroBT Electronics Technology Co., Ltd. This is an English translation of all extracted Chinese body text, headings and captions. Repeated contents entries and page numbers are omitted. The original photographs and screen images remain in the original PDF. Text that exists only inside a photograph or screen image is not covered. AI translation; no human review.

The saved vendor English V1.1 is `vendors-wm-m31s-manual-english-s3`. It is a different specification edition. Its 72 TH/s, 3312 W, P21D power supply and 240 mm height must not replace the values in this Chinese edition. The Chinese heading says M30S; its title and product text say M31S. These source differences are preserved.

## 1. Product introduction (source heading: M30S)

WhatsMiner M31S blockchain server.

| Parameter | Chinese edition value |
|---|---|
| Hash rate | 74 TH/s ±5% |
| Complete miner power | 3450 W ±10% |
| Efficiency | 46 W/T |
| Power supply part number | P21-GB-12-3300-D-V01; AC 200–277 V |
| Ambient temperature | −5 to 35°C |
| Dimensions | 390 × 155 × 230 mm |

## 2. Wiring and rack installation

### 2.1 Wiring precautions

Connect the power supply control cable, adapter board control cable and fan cable correctly. Damage to the control board, hashboard, adapter board or fan from an incorrect connection is outside the source warranty.

Align the latch on each control board cable with its connector. Do not force a reversed plug into a socket. A reversed connection can burn the control board and signal cable when power is applied.

### 2.2 Power supply control cable

This is a 14-pin cable. Align the keyed connector. Page 4 shows the correct and incorrect plug connections.

### 2.3 Adapter board control cable

This is a 22-pin cable. Align the keyed connector. Pages 4–5 show correct and incorrect connections.

### 2.4 Fan cable

This edition specifies a 6-pin fan cable. Align the keyed connector. Page 6 shows correct and incorrect connections.

### 2.5 Hashboard and adapter board

Fully seat the adapter socket on the hashboard pins. Poor contact can cause further faults. Page 7 shows the correct and incorrect connections.

### 2.6 Power supply copper busbars

Do not reverse the positive and negative busbars. Align each screw washer parallel to the busbar edge. A misaligned washer can cause a short circuit and burn the miner when power is applied. Tighten the screws to prevent poor contact and incorrect operation. Page 8 shows correct and incorrect connections.

### 2.7 Complete wiring check

After you connect all cables and tighten all screws, check every connection again.

### 2.8 Handling and rack installation

Connect the power cable at the power supply before you connect its wall socket end. Do not use a data cable, power control cable or fan cable as a lifting handle for the miner or power supply. This can damage or disconnect the cable and physically overload the control board. The source excludes resulting damage and faults from its warranty.

## 3. Configuration environment

### 3.1 Equipment

| Item | Quantity | Use |
|---|---|---|
| Computer | 1 | Configure the miner |
| Miner power supply | 1 | Supply the miner during configuration |
| Network switch | 1 | Connect the miner and computer; the switch must have internet access |
| DHCP/NTP server or router | 1 | Supply the initial IP address and network time |

The factory setting obtains an address through DHCP.

### 3.2 Network

Enable a DHCP server or the router's dynamic address service. Runtime and hash rate statistics depend on NTP time. The factory configuration includes several public NTP servers. The source recommends a local NTP server to improve synchronization speed and accuracy.

## 4. Connections and checks before power-on

1. Follow the warning label on the miner. Shake the miner to check for a loose heatsink or another detached part. Connect and energize it only after you confirm that no part has detached.
2. Connect the power cable and connect Ethernet to the switch. Check that power control, adapter control and fan cables are secure. Check busbar polarity before you apply power.

Do not reverse busbar polarity. Align the washers with the busbar edges and tighten the screws. An incorrect connection can damage a hashboard, control board or power supply; the source excludes reversed-busbar damage from warranty.

Connect the cable between the control board and power supply. Without it, the controller cannot control output voltage and hash rate can be low.

Connect the fan cable securely. A loose fan cable can prevent cooling and cause the miner to reduce clock frequency and hash rate.

## 5. Web configuration

### 5.1 Find the DHCP address

Connect the configuration computer to the same network segment as the miner. Start WhatsMinerTools. Select the tab for IP detection and settings. Enter the room, rack, shelf and position numbers. Select **Start**.

About 30 seconds after power-on, the Ethernet yellow indicator should remain on and the green indicator should flash. Hold the long **IPFOUND** button on the control board for more than 5 seconds. The two right-side indicators flash several times. The miner broadcasts its IP and MAC addresses.

Read the IP address, MAC address and location in WhatsMinerTools.

If all control panel lights are off, check the 220 V and 16 A power cable connections. If the right-side indicators are on but the Ethernet indicator is off or does not flash, check the switch, cable connection and cable quality.

The computer and miner must use the same network segment so the computer can receive the broadcast.

If DHCP is enabled and both devices are on the same segment, but the miner is not detected, hold **Reset** for more than 5 seconds to restore the factory configuration. Power the miner off and on. Wait 30 seconds, then press **IPFOUND** again.

If the tool detects the miner automatically after **Start**, without a button press, the panel can be trapping the IPFOUND button. Find the device by its MAC barcode, turn off its power, and reinstall the control board. Ensure that buttons and indicators pass through their panel openings without being trapped.

### 5.2 Pool, worker and NTP settings

Log in and open the CGMiner configuration page. Set the pool address and worker name. Select **Save & Apply**. Restart CGMiner or reboot the control board to activate the settings.

To restart the mining process, open **Status → CGMiner Status** and select **RestartCGMiner**. To reboot the control board, select **Reboot**, then **Perform reboot**. The login page returns after the system starts, about half a minute later.

To change NTP servers, open **System → System**. The original text calls this a CGMiner configuration page. Add or change the NTP server addresses. Four addresses are set at the factory; replace or supplement them with a local server as needed. Select **Save & Apply**.

### 5.3 Optional static IP address

Open the current DHCP address in a browser. The source gives the factory user `admin` and password `admin`.

Open **Configuration → Interfaces**, then **Edit**. Select protocol **Static address**, then **Switch protocol**. Enter the site's planned IP address, network mask, gateway, broadcast address and DNS server. Select **Save & Apply**.

Use the new static address to log in. The old page otherwise remains in a loading state and eventually fails.

### 5.4 Chinese and English interface

Page 18 shows the language selection screen. No additional body instructions are present.

## 6. Check operation

Log in after you connect the miner to its operating network. Open **Status → CGMiner Status**. Check total hash rate, front and rear fan speeds, connected pools, each board's hash rate and each board's temperature.

At startup, the miner runs an automatic frequency search. The source gives about 40 minutes for this stage. Normal mining begins after this stage. Hash rate during the search can be lower than the normal value.

If the hashboard outlet temperature exceeds 80°C and fan speed is above 7000 rpm, the miner reduces frequency and hash rate. Improve ventilation and cooling. This section says to keep ambient temperature below 40°C; the product table gives a −5 to 35°C range. Both source statements are preserved.

If a fan cable is not secure, the corresponding fan speed can read zero. Excess temperature then causes reduced frequency and hash rate.

If a board or its temperature is not detected, turn off the miner. Reseat that board's power and data connections at both ends, including the control board end. Check that the connections are secure.

## 7. Batch setup, status and firmware upgrade

The source names **WhatsMinerTool 4.2.5** for batch configuration, status checks and firmware upgrades. It refers to the separate WhatsMinerTool operation guide for details.

## 8. Removal and installation

### 8.1 Control board

For removal, disconnect the fan cable from the control board, then remove the four chassis screws. Disconnect the power supply control cable and adapter board control cable. Remove the four bracket screws and take the control board from the bracket. Pages 20–21 show these steps.

For installation, secure the board with the four bracket screws. Connect the power supply and adapter control cables to their correct sockets. Secure the bracket to the chassis with its screws. Connect the fan cable. Page 22 shows the connections.

### 8.2 Power supply

For removal, remove the four screws that fix the supply to the chassis. Remove its six busbar screws. Disconnect the fan cable from the supply. Pull the supply out gently, but not far: its control cable is still attached. Disconnect that cable from the control board and remove the supply. Pages 23–24 show these steps.

For installation, first connect the supply control cable to the correct control board socket. Match the six positive and negative terminals to the busbars. Install and tighten the six screws. Keep each washer parallel to the busbar edge to prevent a short circuit. Tighten the four chassis screws and connect the fan. Pages 24–25 show these steps.

### 8.3 Hashboards

Each miner has three hashboards. Before removal, remove the control board and inlet fan. Remove the adapter board and busbars as shown on page 26. Pull the faulty hashboard out. Record its serial number, or use WeChat's scan function to scan it, and give it to the after-sales technician. Page 27 shows the label.

For installation, support the hashboard with one hand and slide it into the chassis guides. Install the adapter board. Fully seat its socket on the hashboard pins. Install the busbars with correct polarity. Align screw washers parallel to the busbar edges and tighten the screws. A short circuit can burn the miner; poor contact can stop correct operation. Reinstall the control board and fan. Pages 28–29 show these steps.

The source's final note says to insert the power cable connector firmly. A loose connection can overheat and burn the connector.
