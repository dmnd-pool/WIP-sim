//! INA260 conversion and register state, from archived TI SBOS656C.
//! Input voltage and shunt current are supplied by the electrical environment.
//! This component does not infer load from firmware settings.

const NEVER: u64 = u64::MAX;
const TIMES_NS: [u64; 8] = [
    140_000, 204_000, 332_000, 588_000, 1_100_000, 2_116_000, 4_156_000, 8_244_000,
];
const AVERAGES: [u32; 8] = [1, 4, 16, 64, 128, 256, 512, 1024];
const RECOVERY_NS: u64 = 40_000;
const CONFIG: u16 = 0x6127;
const WRITABLE_MASK: u16 = 0xfc03;
const AFF: u16 = 1 << 4;
const CVRF: u16 = 1 << 3;
const OVF: u16 = 1 << 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bus {
    Idle,
    Pointer,
    High,
    Low(u8),
    ReadHigh,
    ReadLow(u8),
    Finished,
}
#[derive(Clone, Copy)]
enum Event {
    Start,
    Current(i16),
    Voltage(u16),
}

/// A bounded ideal sensor in the documented continuous-current operating range.
/// Noise, shunt heating, ADC integration, SCL filtering and SMBus ARA are not
/// supplied by this component. Unsupported bus accesses increment `faults`.
pub struct Sensor {
    config: u16,
    current: i16,
    voltage: u16,
    power: u16,
    mask: u16,
    flags: u16,
    limit: u16,
    vin_uv: u32,
    current_ua: i32,
    due: u64,
    event: Event,
    sample_current: i16,
    sum_current: i64,
    sum_voltage: u64,
    sum_power: u64,
    samples: u32,
    bus: Bus,
    pointer: u8,
    paused_config: bool,
    faults: u64,
    conversions: u64,
    writes: u64,
}
impl Sensor {
    pub fn new(supply_uv: u32, vin_uv: u32, current_ua: i32, now: u64) -> Option<Self> {
        if !(2_700_000..=5_500_000).contains(&supply_uv)
            || !valid_input(vin_uv, current_ua)
            || now > NEVER - TIMES_NS[7]
        {
            return None;
        }
        let mut s = Self {
            config: CONFIG,
            current: 0,
            voltage: 0,
            power: 0,
            mask: 0,
            flags: 0,
            limit: 0,
            vin_uv,
            current_ua,
            due: NEVER,
            event: Event::Start,
            sample_current: 0,
            sum_current: 0,
            sum_voltage: 0,
            sum_power: 0,
            samples: 0,
            bus: Bus::Idle,
            pointer: 0,
            paused_config: false,
            faults: 0,
            conversions: 0,
            writes: 0,
        };
        s.reset(now);
        Some(s)
    }
    pub fn reset(&mut self, now: u64) {
        self.config = CONFIG;
        self.current = 0;
        self.voltage = 0;
        self.power = 0;
        self.mask = 0;
        self.flags = 0;
        self.limit = 0;
        self.pointer = 0;
        self.bus = Bus::Idle;
        self.paused_config = false;
        self.sample_current = 0;
        self.restart(now, false);
    }
    fn active(&self) -> bool {
        self.config & 3 != 0
    }
    fn current_enabled(&self) -> bool {
        self.config & 1 != 0
    }
    fn voltage_enabled(&self) -> bool {
        self.config & 2 != 0
    }
    fn fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
    }
    fn restart(&mut self, now: u64, recover: bool) {
        self.sum_current = 0;
        self.sum_voltage = 0;
        self.sum_power = 0;
        self.samples = 0;
        self.due = NEVER;
        if self.active() {
            if recover {
                self.event = Event::Start;
                self.due = now.saturating_add(RECOVERY_NS);
            } else {
                self.begin(now);
            }
        }
    }
    fn begin(&mut self, now: u64) {
        if self.current_enabled() {
            self.event = Event::Current((self.current_ua / 1250) as i16);
            self.due = now.saturating_add(TIMES_NS[usize::from((self.config >> 3) & 7)]);
        } else {
            self.begin_voltage(now);
        }
    }
    fn begin_voltage(&mut self, now: u64) {
        self.event = Event::Voltage((self.vin_uv / 1250) as u16);
        self.due = now.saturating_add(TIMES_NS[usize::from((self.config >> 6) & 7)]);
    }
    pub fn next_deadline(&self) -> u64 {
        self.due
    }
    pub fn advance(&mut self, now: u64) {
        for _ in 0..4096 {
            if self.due == NEVER || self.due > now {
                return;
            }
            let at = self.due;
            match self.event {
                Event::Start => self.begin(at),
                Event::Current(value) => {
                    self.sample_current = value;
                    self.sum_current += i64::from(value);
                    self.compare(0, value as u16);
                    if self.voltage_enabled() {
                        self.begin_voltage(at);
                    } else {
                        self.finish_sample(at);
                    }
                }
                Event::Voltage(value) => {
                    self.sum_voltage += u64::from(value);
                    // 1.25mA * 1.25mV / 10mW = 1/6400.
                    let p = u64::from(self.sample_current.unsigned_abs()) * u64::from(value) / 6400;
                    if p > 0xa3d7 {
                        self.flags |= OVF;
                        // The datasheet specifies OVF and maximum power but
                        // not the overflow code. Saturation is a visible model
                        // approximation, never a complete physical result.
                        self.fault();
                    } else {
                        self.flags &= !OVF;
                    }
                    let p = p.min(0xa3d7) as u16;
                    self.sum_power += u64::from(p);
                    self.compare(1, value);
                    self.compare(2, p);
                    self.finish_sample(at);
                }
            }
        }
        if self.due != NEVER && self.due <= now {
            self.fault();
            self.due = NEVER;
        }
    }
    fn finish_sample(&mut self, now: u64) {
        self.samples += 1;
        let n = AVERAGES[usize::from((self.config >> 9) & 7)];
        if self.samples == n {
            if self.current_enabled() {
                self.current = (self.sum_current / i64::from(n)) as i16;
            }
            if self.voltage_enabled() {
                self.voltage = (self.sum_voltage / u64::from(n)) as u16;
                self.power = (self.sum_power / u64::from(n)) as u16;
            }
            self.flags |= CVRF;
            self.conversions = self.conversions.saturating_add(1);
            if self.config & 4 == 0 {
                self.due = NEVER;
                return;
            }
            self.restart(now, false);
        } else {
            self.begin(now);
        }
    }
    fn compare(&mut self, kind: u8, value: u16) {
        let selection = self.mask & 0xf800;
        let hit = if selection & 0x8000 != 0 {
            if kind != 0 {
                return;
            }
            (value as i16) > self.limit as i16
        } else if selection & 0x4000 != 0 {
            if kind != 0 {
                return;
            }
            (value as i16) < self.limit as i16
        } else if selection & 0x2000 != 0 {
            if kind != 1 {
                return;
            }
            value > self.limit
        } else if selection & 0x1000 != 0 {
            if kind != 1 {
                return;
            }
            value < self.limit
        } else if selection & 0x0800 != 0 {
            if kind != 2 {
                return;
            }
            value > self.limit
        } else {
            return;
        };
        if hit {
            self.flags |= AFF;
        } else if self.mask & 1 == 0 {
            self.flags &= !AFF;
        }
    }
    /// Logical assertion, before the APOL output-polarity selection.
    pub fn alert(&self) -> bool {
        self.mask & 0xf800 != 0 && self.flags & AFF != 0
            || self.mask & 0x400 != 0 && self.flags & CVRF != 0
    }
    pub fn alert_level(&self) -> bool {
        self.alert() == (self.mask & 2 != 0)
    }
    pub fn set_inputs(&mut self, vin_uv: u32, current_ua: i32, now: u64) -> bool {
        self.advance(now);
        if !valid_input(vin_uv, current_ua) {
            self.fault();
            return false;
        }
        self.vin_uv = vin_uv;
        self.current_ua = current_ua;
        true
    }
    pub fn start(&mut self, read: bool, now: u64) -> bool {
        self.advance(now);
        self.resume_partial(now);
        self.bus = if read { Bus::ReadHigh } else { Bus::Pointer };
        true
    }
    fn resume_partial(&mut self, now: u64) {
        if self.paused_config {
            // An aborted word does not commit a partial configuration. The
            // old conversion restarts; partial-write timing is uncalibrated.
            self.paused_config = false;
            self.restart(now, false);
            self.fault();
        }
    }
    pub fn stop(&mut self, now: u64) {
        self.advance(now);
        self.resume_partial(now);
        self.bus = Bus::Idle;
    }
    pub fn nack(&mut self) {
        self.bus = Bus::Finished;
    }
    pub fn write(&mut self, value: u8, now: u64) -> bool {
        self.advance(now);
        match self.bus {
            Bus::Pointer => {
                if !matches!(value, 0..=3 | 6 | 7 | 0xfe | 0xff) {
                    self.fault();
                    self.bus = Bus::Finished;
                    return false;
                }
                self.pointer = value;
                self.bus = Bus::High;
            }
            Bus::High if matches!(self.pointer, 0 | 6 | 7) => {
                if self.pointer == 0 {
                    self.due = NEVER;
                    self.paused_config = true;
                }
                self.bus = Bus::Low(value);
            }
            Bus::Low(high) => {
                self.commit(u16::from_be_bytes([high, value]), now);
                self.bus = Bus::Finished;
                self.writes = self.writes.saturating_add(1);
            }
            _ => {
                self.fault();
                self.bus = Bus::Finished;
                return false;
            }
        }
        true
    }
    fn commit(&mut self, value: u16, now: u64) {
        match self.pointer {
            0 => {
                self.paused_config = false;
                if value & 0x8000 != 0 {
                    self.reset(now);
                    return;
                }
                let recover = !self.active();
                self.config = 0x6000 | (value & 0xfff);
                if self.mask & 1 != 0 && self.flags & AFF != 0 {
                    // Section8.3.3.2 allows config-write latch clearing;
                    // Table6 says only mask reads clear it. Follow the former
                    // but expose this unresolved reference conflict.
                    self.flags &= !AFF;
                    self.fault();
                }
                if self.active() {
                    self.flags &= !CVRF;
                }
                self.restart(now, recover);
            }
            6 => self.mask = value & WRITABLE_MASK,
            7 => self.limit = value,
            _ => self.fault(),
        }
    }
    fn register(&mut self) -> u16 {
        match self.pointer {
            0 => self.config,
            1 => self.current as u16,
            2 => self.voltage,
            3 => self.power,
            6 => {
                let out = self.mask | self.flags;
                self.flags &= !CVRF;
                if self.mask & 1 != 0 {
                    self.flags &= !AFF;
                }
                out
            }
            7 => self.limit,
            0xfe => 0x5449,
            0xff => 0x2270,
            _ => unreachable!(),
        }
    }
    pub fn read(&mut self, now: u64) -> Option<u8> {
        self.advance(now);
        match self.bus {
            Bus::ReadHigh => {
                let bytes = self.register().to_be_bytes();
                self.bus = Bus::ReadLow(bytes[1]);
                Some(bytes[0])
            }
            Bus::ReadLow(low) => {
                self.bus = Bus::Finished;
                Some(low)
            }
            _ => {
                self.fault();
                None
            }
        }
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
fn valid_input(vin_uv: u32, current_ua: i32) -> bool {
    vin_uv <= 36_000_000 && (-15_000_000..=15_000_000).contains(&current_ua)
}

#[no_mangle]
pub extern "C" fn md_ina260_size() -> usize {
    core::mem::size_of::<Sensor>()
}
/// # Safety
/// State is aligned exclusive writable storage of md_ina260_size bytes.
/// A false return leaves the storage uninitialized.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_init(
    state: *mut Sensor,
    supply_uv: u32,
    vin_uv: u32,
    current_ua: i32,
    now: u64,
) -> bool {
    let Some(s) = Sensor::new(supply_uv, vin_uv, current_ua, now) else {
        return false;
    };
    state.write(s);
    true
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_reset(state: *mut Sensor, now: u64) {
    (*state).reset(now);
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_advance(state: *mut Sensor, now: u64) {
    (*state).advance(now);
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_start(state: *mut Sensor, read: bool, now: u64) -> bool {
    (*state).start(read, now)
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_stop(state: *mut Sensor, now: u64) {
    (*state).stop(now);
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_nack(state: *mut Sensor) {
    (*state).nack();
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_write(state: *mut Sensor, value: u8, now: u64) -> bool {
    (*state).write(value, now)
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_read(state: *mut Sensor, now: u64) -> i32 {
    (*state).read(now).map(i32::from).unwrap_or(-1)
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_inputs(
    state: *mut Sensor,
    vin_uv: u32,
    current_ua: i32,
    now: u64,
) -> bool {
    (*state).set_inputs(vin_uv, current_ua, now)
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_deadline(state: *const Sensor) -> u64 {
    (*state).next_deadline()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_alert_level(state: *const Sensor) -> bool {
    (*state).alert_level()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_faults(state: *const Sensor) -> u64 {
    (*state).faults()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_writes(state: *const Sensor) -> u64 {
    (*state).writes()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_ina260_conversions(state: *const Sensor) -> u64 {
    (*state).conversions()
}

#[cfg(test)]
mod tests;
