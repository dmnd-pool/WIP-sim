# MSKMINER Antminer S9/S9i/S9j Pro 5.1 guide

English translation of the Russian guide. Scope: all extractable Russian body text, tables, and captions on pages 1–14. The final page has no extracted text. Screenshots and their existing English UI labels remain in the original PDF. Image-only text is not certified as fully translated. Date: 2026-10-19.

The performance, temperature, power and life claims below are claims in the historical source. They are not measurements or operating limits verified by this catalogue. Literal UI labels are retained. Some source statements conflict; translator notes identify these points.

## Page 1: scope and installation

The source says that this MSKMINER firmware supports S9, S9i and S9j models. It claims that AsicBoost can save up to 250 W on pools that support AsicBoost, such as Slush Pool, AntPool, BTC.com, EMCD and F2Pool. The interface is similar to the stock interface.

Install by the usual firmware update method. The source excludes Bitmain releases from June 2019 onward from this method and says to use an SD card for those releases. During an upgrade, the source recommends that you clear **Keep settings**.

## Page 2: signature check and initial configuration

If stock firmware reports **Cannot find signature**, the source describes SD recovery. Set JP4 to the installation position on the control board. The source describes removal of the Ethernet front panel, which has two screws, to reach JP4. It points to this OEM reference:
https://support.bitmain.com/hc/en-us/articles/360019493654-S9-series-S9-S9i-S9j-S9-Hydro-Control-Board-Program-Recovery

The firmware permits a separate frequency and voltage for each hashboard. The source advertises profiles above 20 TH/s with air cooling and 19–24 TH/s with immersion cooling. It says that increased output requires a higher-capacity PSU. It describes two stock PSUs per miner, or three 1,600 W stock PSUs for two miners. **The source prohibits connection of one hashboard to two different PSUs.** These configurations are historical vendor statements, not verified electrical designs.

After installation, enter the pool Stratum URL, worker name and password. The source recommends a different provider for the third pool as a reserve. Use **Miner Configurations → General Settings**.

## Pages 3–4: general settings

**Advanced settings** are for experienced users. Defaults necessary for operation are already set.

**Global chain settings** set the default frequency and voltage for all boards. **Individual chains settings (overrides global settings)** sets enable, frequency and voltage separately for each board.

## Page 5: board, fan and watchdog settings

**Setup chain #** contains board settings. Auto Tune sets these values, or an experienced user can change them manually.

**Enabled** enables the hashboard. An overtemperature condition can disable it through **Emergency profile configuration**. The watchdog option **Disable chain if it has less than 63 chips and hashrate is below given number of percents** can also disable it. The source presents this as protection against faults, including damage at six-pin power connectors.

**Frequency** is board frequency in MHz. **Voltage** is board voltage.

Fan and temperature settings:

- **Disable fan check (immersion cooling only!!!)** disables the fan check for immersion operation.
- **Fan speed (auto when unchecked)** sets a fixed fan percentage. Clear it for automatic control, which the source prefers.
- **Low power mode (AsicBoost)** enables AsicBoost. Here the source claims about 200 W savings, rather than the 250 W maximum on page 1.
- **Use new Bitmain fan management profile (increased speed, lower chip temp)** selects a Bitmain fan profile. The Russian explanation calls it a low-noise automatic mode; this conflicts with the English label's increased-speed wording.
- **Prevent chips temperature to fall below this value** maintains a selected chip temperature. The source reports a 75–95 °C operating range and suggests 78–80 °C. These are unverified historical source values, not an OEM maximum-temperature specification.

**Profile → Backup current profile** saves or restores a complete preset.

Watchdog settings:

- **Reboot if temperature is above** reboots above the selected temperature.
- **Restart if hashrate is below given number of percents of ideal hashrate** restarts below the selected percentage.
- **Disable chain if it has less than 63 chips and hashrate is below given number of percents of** disables a chain if both conditions apply.
- **Perform check of red chips** can be off, or run every 30 minutes, every hour, or every three hours. The check starts chip tuning and adjusts individual chip frequencies.

**Blink red and green leds one by one to locate ASIC** locates a unit. The Russian text says both LEDs flash together; the English label says one by one. The actual sequence is not resolved by this source.

## Page 6: LEDs and Auto Tune

- **Blink green led to locate ASIC with number of chips less than 63** signals missing chips.
- **Blink red led to locate ASIC with fan problems** signals a fan fault.
- **Apply LED setup** saves the LED settings.
- **Reset** restores defaults.
- **Save&Apply** saves and applies the settings.

**Auto tuner configuration** sets automatic tuning. **Asic profiles - most typical settings profiles** selects a power/hashrate profile. **Save&Run** saves and starts it. **Advanced settings** opens additional controls.

## Page 7: chain tuning

Profiles are grouped by power consumption. **Chains auto tune** tunes the boards. An experienced user can select a profile and then select **Manual** to edit it.

| UI field | Meaning |
| --- | --- |
| Max frequency of green chips | Maximum frequency for green chips |
| Frequency to | Upper frequency limit |
| ASIC Voltage from | Lower voltage limit |
| ASIC Voltage to | Upper voltage limit |
| CGMiner restart time (seconds) | CGMiner restart interval in seconds |
| Desired hashrate | Target hashrate |
| Statistics poll interval (seconds) | Statistics polling interval |
| Maximum number of restarts | Restart limit |
| Startup config | Initial configuration |
| Initial voltage value | Board voltage at startup |

The source says that tuning seeks the highest frequency within the specified range at the lowest voltage within the specified range, while each board stays close to its ideal hashrate. Manual mode is intended for experienced users.

The source gives a tuning duration of 30–90 minutes. Wait until tuning is complete. It also states that more than six minutes connected to a pool indicates completion; use the explicit log/status checks on page 8 because this time statement alone is ambiguous.

## Page 8: tuning completion and power profiles

The Auto Tune log must show that the process has ended. The message **Auto tune of chips is in progress! Please wait or Kill Auto tunes** must disappear from **Miner General Configuration**. The **Kill Auto tunes** button stops tuning.

The source lists profiles from 680 W to 1,900 W. It says that each profile tunes the voltage and frequency for each board, then tunes each chip. A screenshot is labelled as the result after 60 minutes.

## Page 9: status and fee accounting

**Miner Status** shows miner statistics. Top-right controls:

- **Stop mining** stops mining and enters sleep. The source claims 25–45 W in this state.
- **Search Asic** flashes the front LEDs to locate the unit.
- **Restart miner** restarts mining software.
- **Reboot** restarts the complete device.

The source states a main developer fee of 1.3–1.8%, with a maximum of 2%. It also identifies a reserve developer-fee address for use when the main address is unavailable. The display includes per-board and total power, user **DiffA#**, developer **DiffA#**, and total **DiffA#**.

The source's intended formula is developer work divided by total work, multiplied by 100%. Its printed example, `306 651 022,022 / 4 227 104,104 × 100% = 1.378%`, is arithmetically inconsistent. The numbers are preserved as an identified source error. Do not use that example to validate accounting.

## Page 10: per-chip frequency

**Manual chips frequency config** permits individual frequency changes. The source claims better output and service life and sometimes recovery of a failed board. These are unverified claims. **Chip Auto tune** selects individual frequencies automatically. **Manual** permits individual edits.

Chip colours:

- Red: deviation outside the permitted range.
- Yellow: deviation within the permitted range.
- Green: ideal operation according to the firmware.
- Grey or black: hardware errors. A mouse hover shows the error count.

**Decrease frequency of all red chips** lowers each red chip by one step. **Increase frequency of all green chips** raises each green chip by one step. **Decrease frequency of all HW chips** lowers each chip with hardware errors by one step.

## Page 11: chip errors, additional fee and security

**Show hw If number of hw is more then** sets the displayed hardware-error threshold. The example uses four errors per chip. Darker grey indicates more errors. This page also permits separate voltage control for each hashboard.

Per-board buttons:

- **Green** raises all green chips by one frequency step.
- **Red** lowers all red chips by one step.
- **Yellow** lowers all yellow chips by one step.
- **Gray** lowers all chips with hardware errors by one step.
- **DEL** removes the board's per-chip frequency settings.

**Redirect % of ASIC speed** sets an additional user fee, intended for hosting operators or managers. The source says it runs in parallel with main mining, like the MSKMINER developer fee.

**Security** is the built-in antivirus page.

## Page 12: emergency profile

When the firmware detects a virus, the source says it shows red text and a treatment button.

**Emergency profile** sets a configuration for excessive temperature. Create the profile first, then edit it.

**Load emergency profile if temperature is above (please note that if it doesn't help then system will be rebooted)** selects the threshold for the emergency profile. The source says the system reboots if that profile does not resolve the condition.

**Restore previous configuration after given number of hours of emergency profile work** returns to the previous configuration after the selected interval.

## Page 13: emergency board settings and schedule

**Disable chain with temperature above max** disables a board above the selected maximum. The other emergency board controls match **General Settings**. The source instructs the user to select lower frequency and voltage to reduce board temperature.

**Sheduler** is the source's label for its profile schedule. **Add new** adds a schedule entry. Set its name, start time, weekday and Auto Tune profile. For every day, the source says to leave the day selection unchanged. **Edit config** permits custom settings. Set the correct time zone in **System → Timezone**.

## Page 14: bulk configuration

The firmware can generate a file with pool URLs and Auto Tune settings for other miners. It can append the last two IP-address octets to a worker name. The source example maps worker `Alex` and address `192.168.1.20` to `Alex.1×20`.

**Generate AutoConfig** saves this configuration patch. **Run Auto-tune** starts the selected tuning profile after an update. **Append ip octets to worker** adds the address suffix for identification.

1. Configure the first miner's pools and worker name.
2. Start the desired Auto Tune W/MHz mode.
3. Save **Patch AutoConfig** as a `.tar` file. The source says that tuning does not have to finish first.
4. Upload that file through the firmware update interface of another miner that runs MSKMINER. The configuration contains pool, worker and Auto Tune settings. The worker suffix becomes `.SUBNETxIP`.
5. The source describes either manual upload or use of BTC Tools or Bitmain Toolkit for bulk firmware updates.
6. The source recommends groups of at most 5–10 miners when applying updates or configuration. It cites restart-related phase imbalance on a 380 V supply and load on the site's electrical system. This is a historical source instruction, not a site-specific electrical assessment.
