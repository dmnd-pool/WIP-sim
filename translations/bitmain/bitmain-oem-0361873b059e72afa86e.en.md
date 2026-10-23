# APMinerTool batch-management guide — English translation

Original artifact: `bitmain-oem-0361873b059e72afa86e`.
Original language: Simplified Chinese. The source has 23 pages.
Scope: Complete translation of the extractable body text and tables. The source screenshots remain in the original PDF. Text that exists only inside screenshots is not translated here. Repeated statements are consolidated with their meaning retained. This is a historical guide. Its model and operating-system claims apply to that guide.

## Page 1: Purpose and requirements

The software supports Antminer devices. It does not support other manufacturers. Functions include miner management, fixed IP assignment, firmware upgrade, and settings.

1. The computer must run Windows 7 or later. The source says that XP and “IOS” are not supported. “IOS” is retained as written; the intended platform is unclear.
2. The program selects its language from the operating-system language. A Chinese environment shows Chinese. Other environments show English.
3. The source says to try another browser if the download produces an antivirus warning, and claims that the file is safe. This is a manufacturer claim, not an archive verification result.
4. Extract the software archive before use.
5. The computer and miners must be able to communicate. For fixed IP assignment, they must use the same subnet.

## Pages 1–3: Set fixed IP addresses

Skip this section if the miners already have suitable configured addresses.

1. Open the program. Newly connected miners obtain addresses through DHCP. Fixed addresses can simplify management of many units.
2. Reserve a separate range for static addresses. The source example uses DHCP addresses 101–254 and static addresses 1–100 within one subnet. This prevents address conflicts.
3. Set `Next IP to fill` to the next static address. Select the miner coin type. Use `Pool configuration` to enter pool addresses, worker names, and passwords in a batch.
4. Read the correct subnet mask, gateway, and DNS settings from the computer network configuration. Enter those values in the tool.
5. The source recommends `Automatic configuration`. If it is not selected, enter the next static address manually for each miner.
6. Press each miner `IP report` button for 3–5 seconds, one miner at a time. The tool assigns the addresses in sequence.
7. Select `Configure pool while configuring IP` if required.

## Pages 3–5: Scan and configure miners

Open `Miner management`. Open the IP-range editor. Select `+` to add a range. Double-click an existing range to change it. Save the range.

Scan the range. The tool reports the number of miners in these states: normal, low hash rate, offline, and failed. You can limit the display to successfully detected miners.

To configure miners:

1. Select the coin type. The tool fills the mining-service address automatically. You can change it. Change coin type only while monitoring is stopped.
2. Use Ctrl or `Select all` to select miners of that coin type. Enter the service address, worker name, and password.
3. Select `Configure miners`. The tool reports success or failure. Units of a different coin type can show a configuration failure.
4. Set `Worker-name suffix` as required. The default suffix follows the IP address. Apply the configuration. The result window gives success and failure counts.
5. Select `Automatic miner monitoring`.

## Pages 5–7: Restart, reset, DHCP, and password

To restart a miner, select it and select `Restart miner`. Use Ctrl to select several units. Confirm the action in the dialog.

The source section titled `Restore factory settings` says to select the miner, select `Restart miner`, and confirm. This wording conflicts with the section title. Check the original screenshot and software version before using this instruction. It is not silently corrected here.

To return a static-address miner to DHCP, select it, select `Restore DHCP`, and confirm.

The password button changes miner login passwords in a batch. After the change, update the default password in `Settings > Basic settings` to match. The source example changes `root` to `123456`. Keep a record of the login details. The source says that a forgotten password requires a factory reset to regain access.

## Page 8: Automatic monitoring

After configuration, select `Automatic miner monitoring`. The default refresh interval is five minutes.

| Text colour | State |
| --- | --- |
| Green | Normal |
| Red | Offline |
| Yellow | Low hash rate |
| Blue | Another abnormal parameter |

Select a miner IP address to open its web interface.

## Pages 8–10: Upgrade firmware

The tool can upgrade a batch of miners. The source gives low hash rate as one possible reason to upgrade.

1. Select `Upgrade firmware`.
2. Select the required IP range. Select only the miners to upgrade.
3. Select a firmware file. If needed, use `Firmware download address` to obtain it first. Do not extract the firmware archive.
4. Select `Add firmware`. Wait for `Upload successful`.
5. Keep the default `Keep configuration` option if the settings must remain. Clear it otherwise. The source states that upgrades usually clear the configuration.
6. Select the miner type and firmware name. Select `Upgrade firmware`.
7. Check the miner count in the confirmation dialog. Confirm it.
8. Wait about two minutes. The result dialog reports success or failure.

## Pages 10–11: Settings

The settings window has three sections: `Basic settings`, `Miner alerts`, and `IP-range editor`.

A miner alert occurs when a parameter exceeds its upper or lower threshold. If all parameters are selected, low hash rate has display priority. Other problems show as an abnormal parameter.

The IP-range editor has the same function as the range control in miner management. The source provides screenshots for basic settings but no additional extractable explanation.

## Pages 12–16: Overclock function

Install a firmware version that supports overclocking before use. The source download address is `https://service.bitmain.com.cn/support/download`.

Check the `Profile` column after a scan. A value means that the miner supports this function. You can also check `Working Mode` in the miner web settings.

If the column is empty, the source says that the required overclock firmware is absent. The tool then reports that the selected miner does not support overclocking. The web interface also lacks `Working Mode`.

Select the miner, select `Overclock`, select a profile, and confirm. Wait for the miner to restart. The profile is then applied. Page 15 has no extractable body text.

The guide warns that overclocking has risk. At the time of the guide, this function supports S9-series miners and T9+ only.

| Miner | PSU rating stated by the guide | Additional hash-rate profile stated by the guide |
| --- | --- | --- |
| S9 / S9i / S9j | 1400 W | +0.5 T |
| S9 / S9i / S9j | 1600 W | +1.0 T, +1.5 T, or +2.0 T |
| T9+ | 1600 W | +0.5 T, +1.0 T, or +1.5 T |
| T9+ | 1800 W | +2.0 T |

The source profile labels use “T”. The guide does not provide a measurement uncertainty or a full electrical limit table.

The guide recommends stable operation at `normal` first, then +1.5 T. If +1.5 T works well, it says to consider +2.0 T with regard to PSU and room temperature. If it works poorly, it says to reduce to +1.0 T or +0.5 T.

If the tool reports a wrong password, change its stored miner password under `Settings` to match the miner.

## Pages 17–20: Malware scan

Select an IP range and the miners. Select `Start scan` to detect the units. Then select `Firmware virus scan`.

| Result | Meaning in the guide |
| --- | --- |
| Normal | No infection was detected. |
| Poisoned | Infection was detected. Error details show the infected files. |
| Secure Firmware | Port 22 / SSH is disabled. This scan method cannot operate. |
| Not support — Password Wrong | The tool and miner passwords do not match. Change the password in `Settings > Basic settings`. |
| Not support — SSH failed | The SSH password changed or port 22 is closed. The source recommends SD-card recovery. |
| Not support — `{XXX minertype} Not support` | The model is not supported by this scan method. |

A `Normal` result is the tool result. It does not prove that every possible infection was checked.

## Pages 20–23: Malware removal

Select `Firmware virus removal`, then `One-click download` to obtain the removal firmware. If the file was downloaded before, the button can be absent and the tool proceeds with removal.

When the download finishes, select `Continue removal`. After completion, the tool reports `Operation complete` and `Removal successful`.

| Result | Detail and meaning |
| --- | --- |
| Kill Success | Removal completed successfully. |
| Kill Error — SSH failed | The SSH password changed or port 22 is closed. The source recommends SD-card recovery. |
| Kill Error — Password wrong | Set the tool password to the miner password. |
| Not support — SSH failed | The SSH password changed or port 22 is closed. The source recommends SD-card recovery. |
| Not support — `{XXX minertype} Not support` | The model is not supported. |
| None | No miner was found at the IP address. |

The guide lists these supported models: S9, S9i, S9j, T9, T9+, L3, L3+, L3++, A3, D3, X3, E3, Z9, Z9 mini, R4, DR3, and V9. Non-Bitcoin models in this historical list are supplementary to the current catalogue scope.

An upgrade from an old firmware version can remove worker configuration. Configure the worker again if this occurs.
