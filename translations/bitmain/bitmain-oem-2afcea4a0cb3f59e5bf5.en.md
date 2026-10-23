# AntMiner S1 user manual — English translation

Original artifact: `bitmain-oem-2afcea4a0cb3f59e5bf5`.
Original language: Simplified Chinese. Original revision: 26 November 2013.
Scope: Complete translation of the extractable body text on pages 1–9. The source diagrams and screen images remain in the original PDF. Text that exists only inside those images is not translated here. This is a historical manual, not a new installation instruction.

## Pages 1–2: Title and contents

User manual. Contents: Overview; power connection; network connection; WAN port settings; Wi-Fi settings; pool settings; mining status; system configuration; password settings; firmware upgrade. The source contents and body use different subsection numbers. The headings below follow the body.

## Page 3: Overview

AntMiner S1 is a blade miner. Two blades can form one dual-blade unit. The unit is assembled before shipment. The source photograph shows its parts. One blade can also operate independently. The next photograph shows a single blade.

1. Supply your own ATX power supply with a 12 V DC output, power cables, and an Ethernet cable. Each miner operates at 12 V DC and can require up to 13.5 A. For an ATX supply, the source says to strip the PCI-e power cable and use all three power conductors. If one conductor is used, the source specifies a copper cross-section of 2.5 mm². The text does not clearly distinguish a blade from the complete dual-blade unit in this current statement.
2. Supply a Wi-Fi antenna if you use Wi-Fi. Some control boards can make cgminer unstable in Wi-Fi mode. The manufacturer therefore recommends Ethernet for mining.
3. Do not connect the power cables while they are energized. This prevents a short circuit. Check the completed connections before applying power.

## Page 4: Power connection

Connect a 12 V supply to the miner. Do not reverse the positive and negative connections.

A label on the shipped miner identifies power polarity. Remove this label for use. If polarity is uncertain, check the markings on the PCB. `+12V` is positive. `GND` is negative.

## Pages 4–6: WAN port settings

1. Read the IP-address label on the fan bracket. Set the computer to a valid manual IP address on the same subnet.
2. Connect the miner to the computer with an Ethernet cable.
3. Enter the miner IP address in a browser. Open its management interface. The default user name and password are both `root`.
4. Set the LAN port to DHCP client. Open `Network > Interface > LAN`. Select DHCP client, then select `Switch Protocol`, then `Save & Apply`. Some units have a factory LAN address of `192.168.1.1`. On those units, the WAN port cannot use `192.168.1.x` until this change is made. Otherwise, the miner becomes inaccessible. Skip this step if LAN is already set to DHCP.
5. Set the WAN IP address under `Network > Interface > WAN`. The default configuration assigns a fixed IP address to the miner.

The WAN and LAN ports cannot use the same subnet. Before you set WAN, set LAN to DHCP client or to a different subnet. Later AntMiner versions use DHCP on LAN by default. Keep a record of a fixed address. Otherwise, you can lose access to the miner.

## Pages 6–7: Wi-Fi settings

AntMiner has built-in Wi-Fi. It can connect to another Wi-Fi access point for Internet access and mining. Poor Wi-Fi reception can reduce mining efficiency. Supply your own antenna.

Open `Network > Wifi`. Select `Scan`. Select and add an available access point.

## Page 7: Pool settings

Open `Status > Miner Configuration`.

- `Pool URL`: Enter the pool address.
- `Worker`: Enter the worker registered with that pool.
- `Password`: Enter the worker password. The source states that many pools accept an arbitrary value for this field.

You can configure three pools. Priority decreases from Pool 1 to Pool 3. When a higher-priority worker stops, a lower-priority worker starts automatically.

## Page 8: Mining status

Open `Status > Miner Status` to view operation.

- `ASIC#` is the number of chips on a chain.
- `Fan` is fan speed.
- `Temp` is temperature in degrees Celsius.
- `Frequency` is ASIC operating frequency.
- In `ASIC status`, `o` means OK. `x` means that the chip is not operating.

## Page 8: Password settings

Open `System > Administration` to change the login password. Select `Save` or `Save and Apply` after the change. Keep a record of the password. Otherwise, you can lose access to the management interface.

## Pages 8–9: Firmware upgrade

Open `System > Backup / Flash Firmware`.

`Keep setting` is selected by default. Keep this option selected to retain the current settings. Clear it to restore factory settings.

Select `Browse` and choose the firmware file. Select `Flash image...` to upload it. After the upload, select `Proceed` to start the upgrade. Wait and keep power connected. The source states that an interruption can restore the unit to its factory state. After a successful upgrade, the browser returns to the login screen.

Keep a record of the final miner IP address and login password.
