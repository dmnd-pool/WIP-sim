//! EMC2101-R host-load mode, from archived Microchip DS20006703A.
//! Temperatures are explicit physical inputs in eighths of a degree Celsius.
//! Tachometer measurements come only from supplied signal edges.

const STARTUP_NS: u64 = 10_000_000;
const INTERNAL_NS: u64 = 3_000_000;
const EXTERNAL_NS: u64 = 21_000_000;
const NEVER: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diode {
    Temperature(i16),
    Open,
    Short,
}
impl Diode {
    fn sample(self) -> (i16, bool) {
        match self {
            // Table4-2 reserves1023 for a short fault. Its upper saturation
            // row maps >=127.875C to1022, even though the temperature column
            // and represented code differ by one eighth of a degree.
            Self::Temperature(v) => (v.clamp(-512, 1022), false),
            Self::Open => (127 * 8, true),
            Self::Short => (127 * 8 + 7, false),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Pointer,
    Write,
    Read,
    Finished,
}
#[derive(Clone, Copy)]
enum Event {
    Start,
    Internal,
    External,
}

/// A partial physical component. Unsupported operating modes fail visibly;
/// they are not implemented by accepting writes without their effects.
pub struct Sensor {
    regs: [u8; 256],
    supply_uv: u32,
    boot_full: bool,
    internal: i16,
    external: Diode,
    sample_internal: i16,
    sample_external: (i16, bool),
    due: u64,
    event: Event,
    cycle_start: u64,
    ready_at: u64,
    phase: Phase,
    pointer: Option<u8>,
    ext_shadow: u8,
    tach_shadow: u8,
    status: u8,
    alarm_conditions: u8,
    queue: [u8; 3],
    critical: bool,
    tcrit_written: bool,
    alert_latched: bool,
    spin_until: u64,
    fan_written: bool,
    tach_level: bool,
    tach_first: Option<u64>,
    tach_edges: u8,
    tach_due: u64,
    conversions: u64,
    faults: u64,
    writes: u64,
}
impl Sensor {
    /// Host-load strap: false is 10 kOhm/0%; true is 5.6 kOhm/100%.
    /// Auto-load EEPROM straps are not supported by this constructor.
    pub fn new(
        supply_uv: u32,
        boot_full: bool,
        internal: i16,
        external: Diode,
        now: u64,
    ) -> Option<Self> {
        if !(3_000_000..=3_600_000).contains(&supply_uv)
            || !(-320..=1000).contains(&internal)
            || matches!(external, Diode::Temperature(v) if !(-1024..=1023).contains(&v))
            || now > NEVER - STARTUP_NS
        {
            return None;
        }
        let mut s = Self {
            regs: [0; 256],
            supply_uv,
            boot_full,
            internal,
            external,
            sample_internal: internal,
            sample_external: external.sample(),
            due: NEVER,
            event: Event::Start,
            cycle_start: now,
            ready_at: now,
            phase: Phase::Idle,
            pointer: None,
            ext_shadow: 0,
            tach_shadow: 0xff,
            status: 0,
            alarm_conditions: 0,
            queue: [0; 3],
            critical: false,
            tcrit_written: false,
            alert_latched: false,
            spin_until: 0,
            fan_written: false,
            tach_level: false,
            tach_first: None,
            tach_edges: 0,
            tach_due: NEVER,
            conversions: 0,
            faults: 0,
            writes: 0,
        };
        s.power_reset(now);
        Some(s)
    }
    pub fn power_reset(&mut self, now: u64) {
        self.regs = [0; 256];
        for (reg, value) in [
            (4, 8),
            (5, 70),
            (7, 70),
            (0x16, 0xa4),
            (0x17, 0x12),
            (0x18, 8),
            (0x19, 85),
            (0x21, 10),
            (0x46, 0xff),
            (0x47, 0xff),
            (0x48, 0xff),
            (0x49, 0xff),
            (0x4a, 0x20 | if self.boot_full { 0x10 } else { 0 }),
            (0x4b, 0x3f),
            (0x4d, 0x17),
            (0x4e, 1),
            (0x4f, 4),
            (0xfd, 0x28),
            (0xfe, 0x5d),
            (0xff, 1),
        ] {
            self.regs[reg] = value;
        }
        for reg in (0x50..=0x5e).step_by(2) {
            self.regs[reg] = 0x7f;
            self.regs[reg + 1] = 0x3f;
        }
        self.ready_at = now.saturating_add(STARTUP_NS);
        self.due = self.ready_at;
        self.event = Event::Start;
        self.phase = Phase::Idle;
        self.pointer = None;
        self.ext_shadow = 0;
        self.tach_shadow = 0xff;
        self.status = 0;
        self.alarm_conditions = 0;
        self.queue = [0; 3];
        self.critical = false;
        self.tcrit_written = false;
        self.alert_latched = false;
        self.spin_until = 0;
        self.fan_written = false;
        self.tach_first = None;
        self.tach_edges = 0;
        self.tach_level = false;
        self.tach_due = NEVER;
    }
    fn period(&self) -> u64 {
        16_000_000_000u64 >> self.regs[4].min(9)
    }
    fn begin_conversion(&mut self, now: u64) {
        self.sample_internal = self.internal;
        self.sample_external = self.external.sample();
        self.cycle_start = now;
        self.status |= 0x80;
        self.event = Event::Internal;
        self.due = now.saturating_add(INTERNAL_NS);
    }
    pub fn next_deadline(&self) -> u64 {
        self.due.min(self.tach_due)
    }
    pub fn advance(&mut self, now: u64) {
        if self.tach_due != NEVER && self.tach_due <= now {
            self.regs[0x46] = 0xff;
            self.regs[0x47] = 0xff;
            self.status |= 1;
            self.alarm_conditions |= 1;
            if self.regs[0x4a] & 0x0c == 0 || self.regs[0x4a] & 3 == 0 {
                // The documented low-speed false-reading mode is not a valid
                // RPM oracle. Expose that missing behavior as a model fault.
                self.faults = self.faults.saturating_add(1);
            }
            self.tach_first = None;
            self.tach_edges = 0;
            self.tach_due = NEVER;
            self.update_alert();
        }
        // Each pending event is real virtual-time work. A caller must not omit
        // arbitrarily many callbacks and then claim a complete sensor trace.
        for _ in 0..256 {
            if self.due == NEVER || self.due > now {
                return;
            }
            let at = self.due;
            match self.event {
                Event::Start => self.begin_conversion(at),
                Event::Internal => {
                    self.regs[0] = self.sample_internal.div_euclid(8) as i8 as u8;
                    self.event = Event::External;
                    self.due = at.saturating_add(EXTERNAL_NS);
                }
                Event::External => {
                    let raw = (self.sample_external.0 as u16) << 5;
                    self.regs[1] = (raw >> 8) as u8;
                    self.regs[0x10] = raw as u8;
                    self.status &= !0x80;
                    self.conversions = self.conversions.saturating_add(1);
                    self.update_limits();
                    if self.regs[3] & 0x40 != 0 {
                        self.due = NEVER;
                    } else {
                        self.event = Event::Start;
                        self.due = self.cycle_start.saturating_add(self.period());
                    }
                }
            }
        }
        if self.due != NEVER && self.due <= now {
            self.faults = self.faults.saturating_add(1);
            self.due = NEVER;
            self.status &= !0x80;
        }
    }
    fn update_limits(&mut self) {
        let temp = self.sample_external.0;
        let high = i16::from(self.regs[7]) * 8 + i16::from(self.regs[0x13] >> 5);
        let low = i16::from(self.regs[8]) * 8 + i16::from(self.regs[0x14] >> 5);
        let critical = i16::from(self.regs[0x19]) * 8;
        if temp >= critical {
            self.critical = true;
        }
        if temp < critical - i16::from(self.regs[0x21]) * 8 {
            self.critical = false;
        }
        let conditions = [temp > high, temp < low, self.critical];
        let mut flags = if self.sample_internal >= i16::from(self.regs[5]) * 8 {
            0x40
        } else {
            0
        };
        if self.sample_external.1 {
            flags |= 4;
        }
        for (i, bit) in [0x10, 8, 2].into_iter().enumerate() {
            self.queue[i] = if conditions[i] {
                self.queue[i].saturating_add(1)
            } else {
                0
            };
            let threshold = if self.regs[3] & 1 != 0 { 3 } else { 1 };
            if self.queue[i] >= threshold {
                flags |= bit;
            }
        }
        self.status = (self.status & 0xa1) | flags;
        self.alarm_conditions = (self.alarm_conditions & 1) | flags;
        self.update_alert();
    }
    fn update_alert(&mut self) {
        let active = self.alarm_conditions & !self.regs[0x16] & 0x5b != 0;
        if self.regs[0xbf] & 1 != 0 {
            self.alert_latched = active;
        } else if active {
            self.alert_latched = true;
        }
    }
    pub fn alert(&self) -> bool {
        self.regs[3] & 4 == 0
            && self.alert_latched
            && (self.regs[0xbf] & 1 != 0 || self.regs[3] & 0x80 == 0)
    }
    /// Raw fan request only. It is never a tachometer or physical RPM result.
    /// PWM waveform, DAC loading and a fan's mechanical response are not yet
    /// connected. Callers must not use this value as observed fan speed.
    pub fn fan_setting(&self, now: u64) -> u8 {
        if self.regs[3] & 0x60 == 0x60 {
            return if self.boot_full { 63 } else { 0 };
        }
        if self.critical {
            return 63;
        }
        if !self.fan_written {
            return if self.boot_full { 63 } else { 0 };
        }
        if now < self.spin_until {
            if self.regs[0x4b] & 0x20 != 0 && self.regs[3] & 4 != 0 {
                return 63;
            }
            return [0, 32, 48, 63][usize::from((self.regs[0x4b] >> 3) & 3)];
        }
        self.regs[0x4c]
    }
    pub fn set_inputs(&mut self, internal: i16, external: Diode, now: u64) -> bool {
        if !(-320..=1000).contains(&internal)
            || matches!(external, Diode::Temperature(v) if !(-1024..=1023).contains(&v))
        {
            return false;
        }
        self.advance(now);
        self.internal = internal;
        self.external = external;
        true
    }
    /// Both rising and falling edges are supplied. Five edges span one fan
    /// revolution for the datasheet's two-pulses/revolution input contract.
    pub fn tach_level(&mut self, level: bool, now: u64) {
        self.advance(now);
        if level == self.tach_level {
            return;
        }
        self.tach_level = level;
        if self.regs[3] & 4 == 0 {
            return;
        }
        let Some(first) = self.tach_first else {
            self.tach_first = Some(now);
            self.tach_edges = 1;
            self.tach_due = now.saturating_add(728_166_667);
            return;
        };
        if now <= first {
            self.fail();
            return;
        }
        self.tach_edges += 1;
        if self.tach_edges < 5 {
            return;
        }
        let count = u128::from(now - first) * 90_000 / 1_000_000_000;
        self.tach_first = Some(now);
        self.tach_edges = 1;
        self.tach_due = now.saturating_add(728_166_667);
        if count == 0 || count > 65535 {
            // Under-range wrap behavior depends on the PWM clock and is not
            // specified fully enough to report a fabricated speed.
            self.fail();
            return;
        }
        let count = count as u16;
        let limit = u16::from_le_bytes([self.regs[0x48], self.regs[0x49]]);
        self.status = (self.status & !1) | u8::from(count > limit);
        self.alarm_conditions = (self.alarm_conditions & !1) | u8::from(count > limit);
        let saturate = self.regs[0x4a] & 0x0c != 0 && self.regs[0x4a] & 3 != 0 && count > limit;
        let raw = if saturate { u16::MAX } else { count }.to_le_bytes();
        self.regs[0x46] = raw[0];
        self.regs[0x47] = raw[1];
        if count < limit && self.regs[0x4b] & 0x20 != 0 {
            self.spin_until = now;
        }
        self.update_alert();
    }
    fn canonical(reg: u8) -> u8 {
        match reg {
            9 => 3,
            0xa => 4,
            0xb => 5,
            0xd => 7,
            0xe => 8,
            _ => reg,
        }
    }
    fn known(reg: u8) -> bool {
        matches!(reg, 0..=5 | 7..=0x14 | 0x16..=0x19 | 0x21 | 0x46..=0x5f | 0xbf | 0xfd..=0xff)
    }
    fn fail(&mut self) -> bool {
        self.faults = self.faults.saturating_add(1);
        self.phase = Phase::Finished;
        false
    }
    pub fn start(&mut self, read: bool, now: u64) -> bool {
        self.advance(now);
        if now < self.ready_at || !(3_000_000..=3_600_000).contains(&self.supply_uv) {
            return false;
        }
        if read && self.pointer.is_none() {
            return self.fail();
        }
        self.phase = if read { Phase::Read } else { Phase::Pointer };
        true
    }
    pub fn stop(&mut self) {
        self.phase = Phase::Idle;
    }
    pub fn nack(&mut self) {
        self.phase = Phase::Finished;
    }
    pub fn write(&mut self, value: u8, now: u64) -> bool {
        self.advance(now);
        if self.phase == Phase::Pointer {
            let reg = Self::canonical(value);
            if !Self::known(reg) {
                self.pointer = None;
                return self.fail();
            }
            self.pointer = Some(reg);
            self.phase = Phase::Write;
            return true;
        }
        if self.phase != Phase::Write {
            return self.fail();
        }
        let Some(reg) = self.pointer else {
            return self.fail();
        };
        let old = self.regs[reg as usize];
        let value = match reg {
            3 => value,
            4 => value & 15,
            5 | 7 | 8 | 0x19 | 0x21 | 0x4f => value & 0x7f,
            0xc | 0x11 | 0x12 | 0x48 | 0x49 | 0x4e => value,
            0xf => 0,
            0x13 | 0x14 => value & 0xe0,
            0x16 => value | 0xa4,
            0x17 => value & 0x3f,
            0x18 => value & 15,
            0x4a => value & 0x7f,
            0x4b | 0x4c => value & 0x3f,
            0x4d => value & 0x1f,
            0x50..=0x5f => {
                if reg & 1 == 0 {
                    value & 0x7f
                } else {
                    value & 0x3f
                }
            }
            0xbf => value & 7,
            _ => return self.fail(),
        };
        // No quiet emulation of missing LUT/filter/analog compensation modes.
        if (reg == 0x4a && value & 0x20 == 0)
            || (reg == 0xbf && value & 6 != 0)
            || (reg == 0x17 && value != 0x12)
            || (reg == 0x18 && value != 8)
        {
            return self.fail();
        }
        if reg == 0x19 && (self.regs[3] & 2 == 0 || self.tcrit_written) {
            self.phase = Phase::Finished;
            return true; // The part locks TCRIT until explicitly unlocked once.
        }
        self.regs[reg as usize] = value;
        self.writes = self.writes.saturating_add(1);
        self.phase = Phase::Finished;
        match reg {
            3 => {
                if value & 0x40 != 0 {
                    self.due = NEVER;
                    self.status &= !0x80;
                } else if old & 0x40 != 0 {
                    self.begin_conversion(now);
                }
                if value & 4 == 0 {
                    self.tach_first = None;
                    self.tach_edges = 0;
                    self.tach_due = NEVER;
                }
            }
            0xf if self.regs[3] & 0x40 != 0 => self.begin_conversion(now),
            0x19 => self.tcrit_written = true,
            0x4c => {
                self.fan_written = true;
                let duration =
                    [0, 50, 100, 200, 400, 800, 1600, 3200][usize::from(self.regs[0x4b] & 7)];
                if old == 0
                    && value > 0
                    && (self.regs[0x4b] & 0x18 != 0
                        || (self.regs[3] & 4 != 0 && self.regs[0x4b] & 0x20 != 0))
                {
                    self.spin_until = now.saturating_add(duration * 1_000_000);
                }
                if value == 0 {
                    self.spin_until = 0;
                }
            }
            _ => {}
        }
        self.update_alert();
        true
    }
    pub fn read(&mut self, now: u64) -> Option<u8> {
        self.advance(now);
        if self.phase != Phase::Read {
            self.fail();
            return None;
        }
        let reg = self.pointer?;
        self.phase = Phase::Finished;
        Some(match reg {
            1 => {
                self.ext_shadow = self.regs[0x10];
                self.regs[1]
            }
            0x10 => self.ext_shadow,
            0x46 => {
                self.tach_shadow = self.regs[0x47];
                self.regs[0x46]
            }
            0x47 => self.tach_shadow,
            2 => {
                let status = self.status;
                if self.regs[0xbf] & 1 == 0 {
                    if status & 0x7b != 0 {
                        self.regs[3] |= 0x80;
                        self.alert_latched = false;
                    }
                    self.status &= 0x80;
                } else {
                    self.status &= !0x24;
                    self.alert_latched = false;
                }
                status
            }
            _ => self.regs[usize::from(reg)],
        })
    }
    pub fn faults(&self) -> u64 {
        self.faults
    }
    pub fn writes(&self) -> u64 {
        self.writes
    }
    pub fn conversions(&self) -> u64 {
        self.conversions
    }
}

fn diode(kind: u32, eighths: i32) -> Option<Diode> {
    match kind {
        0 => Some(Diode::Open),
        1 => Some(Diode::Short),
        2 if (-1024..=1023).contains(&eighths) => Some(Diode::Temperature(eighths as i16)),
        _ => None,
    }
}
#[no_mangle]
pub extern "C" fn md_emc2101_size() -> usize {
    core::mem::size_of::<Sensor>()
}
/// # Safety
/// State is aligned exclusive writable storage of md_emc2101_size bytes.
/// A false return leaves the storage uninitialized.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_init(
    state: *mut Sensor,
    supply_uv: u32,
    boot_full: bool,
    internal: i32,
    external_kind: u32,
    external: i32,
    now: u64,
) -> bool {
    let (Ok(internal), Some(external)) = (i16::try_from(internal), diode(external_kind, external))
    else {
        return false;
    };
    let Some(sensor) = Sensor::new(supply_uv, boot_full, internal, external, now) else {
        return false;
    };
    state.write(sensor);
    true
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_reset(state: *mut Sensor, now: u64) {
    (*state).power_reset(now);
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_advance(state: *mut Sensor, now: u64) {
    (*state).advance(now);
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_start(state: *mut Sensor, read: bool, now: u64) -> bool {
    (*state).start(read, now)
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_write(state: *mut Sensor, value: u8, now: u64) -> bool {
    (*state).write(value, now)
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_read(state: *mut Sensor, now: u64) -> i32 {
    (*state).read(now).map(i32::from).unwrap_or(-1)
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_stop(state: *mut Sensor) {
    (*state).stop();
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_nack(state: *mut Sensor) {
    (*state).nack();
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_tach(state: *mut Sensor, level: bool, now: u64) {
    (*state).tach_level(level, now);
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_inputs(
    state: *mut Sensor,
    internal: i32,
    external_kind: u32,
    external: i32,
    now: u64,
) -> bool {
    let (Ok(internal), Some(external)) = (i16::try_from(internal), diode(external_kind, external))
    else {
        return false;
    };
    (*state).set_inputs(internal, external, now)
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_deadline(state: *const Sensor) -> u64 {
    (*state).next_deadline()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_faults(state: *const Sensor) -> u64 {
    (*state).faults()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_writes(state: *const Sensor) -> u64 {
    (*state).writes()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_conversions(state: *const Sensor) -> u64 {
    (*state).conversions()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_alert(state: *const Sensor) -> bool {
    (*state).alert()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_emc2101_fan_setting(state: *const Sensor, now: u64) -> u8 {
    (*state).fan_setting(now)
}

#[cfg(test)]
mod tests;
