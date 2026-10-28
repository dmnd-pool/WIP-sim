//! Explicit test stimulus for the live selected-logic component.
//! These timestamps do not model CPU instruction or bus-stall timing.
use super::live_child::LiveChild;
use super::live_protocol::{Kind, Logic, Op, Request, Response};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

struct Stimulus<'a> {
    child: &'a mut LiveChild,
    sequence: u64,
    time_ps: u64,
    transactions: u64,
    receipts: Vec<Value>,
}

impl Stimulus<'_> {
    fn command(&mut self, operation: Op, args: [u64; 12], delta_ps: u64) -> Result<Response> {
        self.sequence = self.sequence.checked_add(1).context("sequence overflow")?;
        let at_ps = self
            .time_ps
            .checked_add(delta_ps)
            .context("stimulus time overflow")?;
        let request = Request {
            sequence: self.sequence,
            at_ps,
            operation,
            args,
        };
        let responses = self.child.transact(&request)?;
        let completion = responses.last().context("live completion absent")?;
        ensure!(
            completion.kind == Kind::Complete,
            "last response is not completion"
        );
        self.time_ps = completion.at_ps;
        self.receipts.push(
            json!({"sequence":request.sequence,"requested_time_ps":at_ps,
            "operation":operation as u64,"arguments":args,"completed_time_ps":completion.at_ps,
            "completion_fields":completion.args,"interval_events":responses.len()-1}),
        );
        Ok(completion.clone())
    }
    fn drive(&mut self, period: Option<u64>, fabric_reset: u64, axi_reset: u64) -> Result<()> {
        let mut args = [0; 12];
        args[4] = fabric_reset;
        args[5] = axi_reset;
        args[8] = 0x30;
        if let Some(period) = period {
            args[1] = period;
            args[8] |= 2;
        }
        self.command(Op::Drive, args, 1)?;
        Ok(())
    }
    fn advance(&mut self, delta_ps: u64) -> Result<Response> {
        self.command(Op::Advance, [0; 12], delta_ps)
    }
    fn read(&mut self, address: u32) -> Result<u32> {
        let mut args = [0; 12];
        args[0] = u64::from(address);
        let completion = self.command(Op::Read32, args, 1)?;
        self.transactions += 1;
        Ok(Logic::unpack(completion.args[0], 32)?.known()?)
    }
    fn write(&mut self, address: u32, value: u32, strobe: u8) -> Result<()> {
        let mut args = [0; 12];
        args[..3].copy_from_slice(&[u64::from(address), u64::from(value), u64::from(strobe)]);
        self.command(Op::Write32, args, 1)?;
        self.transactions += 1;
        Ok(())
    }
    fn reset(&mut self, first: bool) -> Result<Response> {
        // This is explicit100MHz component stimulus, matching the prior fixture.
        // The live interface itself has no default clock rate.
        self.drive(first.then_some(10u64 << 32), 0, 0)?;
        self.advance(1_000_000)?;
        self.drive(None, 15, 1)?;
        self.advance(1_000_000)
    }
}

fn word(value: &Value) -> Result<u32> {
    Ok(u32::try_from(
        value.as_u64().context("contract word absent")?,
    )?)
}

pub fn selected(child: &mut LiveChild, contract: &Value) -> Result<Value> {
    let ready = child.ready()?;
    ensure!(
        ready.last().is_some_and(|r| r.kind == Kind::Ready),
        "transport initialization absent"
    );
    let bases = contract["common_bases"]
        .as_array()
        .context("contract bases")?
        .iter()
        .map(word)
        .collect::<Result<Vec<_>>>()?;
    ensure!(bases.len() == 3, "selected chain count");
    let version_offset = word(&contract["common_offsets"]["version"])?;
    let build_offset = word(&contract["common_offsets"]["build_timestamp"])?;
    let control_offset = word(&contract["common_offsets"]["control"])?;
    let baud_offset = word(&contract["common_offsets"]["baud_divisor"])?;
    let version_mask = word(&contract["version"]["mask"])?;
    let version_expected = word(&contract["version"]["expected"])?;
    let build_min = word(&contract["build_timestamp"]["minimum_inclusive"])?;
    let build_max = word(&contract["build_timestamp"]["maximum_inclusive"])?;
    let control_mask = word(&contract["control_startup"]["clear_then_set_mask"])?;
    let mut sim = Stimulus {
        child,
        sequence: 0,
        time_ps: 0,
        transactions: 0,
        receipts: Vec::new(),
    };
    let first_reset = sim.reset(true)?;
    let irq = Logic::unpack(first_reset.args[3], 20)?;
    // These ten actual fabric connections must be defined after reset.
    // Other bits are retained as observed; their values are never substituted.
    ensure!(
        irq.bval & 0x3ff == 0,
        "used selected IRQ signals remain unknown after reset"
    );
    let mut original = Vec::new();
    for &base in &bases {
        let at = |offset| {
            base.checked_add(offset)
                .context("contract address overflow")
        };
        let version = sim.read(at(version_offset)?)?;
        ensure!(
            version & version_mask == version_expected,
            "original firmware version predicate"
        );
        let build = sim.read(at(build_offset)?)?;
        ensure!(
            (build_min..=build_max).contains(&build),
            "original firmware build predicate"
        );
        let control = sim.read(at(control_offset)?)?;
        for value in [control & !control_mask, control | control_mask] {
            sim.write(at(control_offset)?, value, 15)?;
            ensure!(
                sim.read(at(control_offset)?)? & control_mask == value & control_mask,
                "actual control update differs"
            );
        }
        let baud = sim.read(at(baud_offset)?)?;
        for (value, strobe, expected) in [
            (7, 15, 7),
            (17, 2, 7),
            (17, 1, 17),
            (31, 0, 17),
            (baud, 15, baud),
            (baud ^ 1, 15, baud ^ 1),
        ] {
            sim.write(at(baud_offset)?, value, strobe)?;
            ensure!(
                sim.read(at(baud_offset)?)? == expected,
                "actual baud/strobe response differs"
            );
        }
        original.push((version, build, control, baud));
    }
    ensure!(sim.transactions == 60, "pre-reset transaction count");
    let second_reset = sim.reset(false)?;
    ensure!(
        Logic::unpack(second_reset.args[3], 20)?.bval & 0x3ff == 0,
        "used IRQ signals unknown after second reset"
    );
    for (&base, &(version, build, control, baud)) in bases.iter().zip(&original) {
        for (offset, expected) in [
            (version_offset, version),
            (build_offset, build),
            (baud_offset, baud),
            (control_offset, control),
        ] {
            ensure!(
                sim.read(base.checked_add(offset).context("address overflow")?)? == expected,
                "actual reset register result differs"
            );
        }
    }
    ensure!(sim.transactions == 72, "live transaction count");
    sim.command(Op::Stop, [0; 12], 1)?;
    Ok(
        json!({"schema":"md-sim-selected-live-pl-component/1","passed":true,
        "transactions":sim.transactions,"reset_sequences":2,"commands":sim.receipts,
        "initial_registers":original,"first_reset_irq":first_reset.args[3],
        "second_reset_irq":second_reset.args[3],
        "test_clock_hz":100000000,"interface_default_clock_hz":null,
        "stimulus_time_policy":"Each new test command has an explicit future timestamp; the next sequential input is1ps after actual prior completion. Reset holds/releases each run for1us. These are component test inputs, not CPU timing.",
        "transport_ready_is_firmware_ready":false,"firmware_executed":false,
        "cpu_synchronization_complete":false,"original_board_gate":false,
        "physical_timing_calibrated":false}),
    )
}

/// Independently join the unchanged task's text observations to the wire
/// receipts. Never search past unknown diagnostics or remove the PS7 notice.
pub fn verify_stdout(stdout: &str, scenario: &Value) -> Result<()> {
    let mut expected = vec![super::axi::expected_notice()];
    for command in scenario["commands"]
        .as_array()
        .context("command evidence")?
    {
        let op = command["operation"]
            .as_u64()
            .context("operation evidence")?;
        if op != Op::Read32 as u64 && op != Op::Write32 as u64 {
            continue;
        }
        let argument = |index| {
            command["arguments"][index]
                .as_u64()
                .context("argument evidence")
        };
        let field = |index| {
            command["completion_fields"][index]
                .as_u64()
                .context("completion evidence")
        };
        let address = u32::try_from(argument(0)?)?;
        let id = u16::try_from(field(2)?)?;
        let response = Logic::unpack(field(1)?, 2)?.known()?;
        if op == Op::Read32 as u64 {
            let data = Logic::unpack(field(0)?, 32)?.known()?;
            expected.push(format!("AXI_READ_OBS addr={address:08x} id={id:03x} data={data:08x} resp={response:02b} last=1"));
        } else {
            let data = u32::try_from(argument(1)?)?;
            let strobe = u8::try_from(argument(2)?)?;
            expected.push(format!("AXI_WRITE_OBS addr={address:08x} id={id:03x} data={data:08x} strobe={strobe:x} resp={response:02b}"));
        }
    }
    ensure!(expected.len() == 73, "live text observation count");
    ensure!(
        stdout.lines().eq(expected.iter().map(String::as_str)),
        "live wire/text evidence differs or contains an unknown diagnostic"
    );
    Ok(())
}

pub fn event_regions(child: &mut LiveChild) -> Result<Value> {
    let ready = child.ready()?;
    let initialized = ready.last().context("event fixture ready")?;
    ensure!(
        initialized.kind == Kind::Ready && initialized.at_ps == 0,
        "event fixture initial phase"
    );
    // At time0: first0 -> second7 -> (7 XOR85)+3 =85.
    ensure!(
        Logic::unpack(initialized.args[4], 19)?.known()? == 85,
        "initial NBA/continuous work did not settle"
    );
    let response = child.transact(&Request {
        sequence: 1,
        at_ps: 1000,
        operation: Op::Advance,
        args: [0; 12],
    })?;
    let completed = response.last().context("event fixture completion")?;
    // At exactly1000ps: first9 -> second16 -> (16 XOR85)+3 =72.
    ensure!(
        completed.kind == Kind::Complete && completed.at_ps == 1000,
        "adapter advanced time to hide a delta-cycle error"
    );
    ensure!(
        Logic::unpack(completed.args[0], 32)?.known()? == 72,
        "completion sampled before the NBA chain settled"
    );
    ensure!(
        completed.args[3] == (3 | (6u64 << 32)),
        "settled IRQ X/Z differs"
    );
    let irq_events: Vec<_> = response
        .iter()
        .filter(|r| r.kind == Kind::Irq)
        .map(|r| (r.sequence, r.at_ps, r.args[0]))
        .collect();
    ensure!(
        irq_events == [(1, 1000, 5 | (6u64 << 32)), (1, 1000, 3 | (6u64 << 32))],
        "same-time raw IRQ transitions, X/Z values or command IDs differ"
    );
    ensure!(
        response
            .iter()
            .any(|r| r.kind == Kind::Output && r.at_ps == 1000 && r.args[0] == 72),
        "raw output transition absent"
    );
    child.transact(&Request {
        sequence: 2,
        at_ps: 1001,
        operation: Op::Stop,
        args: [0; 12],
    })?;
    Ok(json!({"passed":true,"initial_value":85,"settled_value":72,
        "command_and_completion_time_ps":1000,"final_x_bits":2,"final_z_bits":4,
        "same_time_irq_transitions":irq_events,
        "artificial_settling_delay_ps":0,"firmware_executed":false}))
}

/// At15001ps the old clock rises, while the same command changes the period,
/// reset and input. Old-edge sampling must win in both command partitions.
pub fn clock_boundary(child: &mut LiveChild, split: bool, unknown_start: bool) -> Result<Value> {
    let mut observed = child.ready()?;
    let offset = if unknown_start { 200 } else { 0 };
    let mut sequence = 0;
    let mut command = |at_ps, operation, args| -> Result<Response> {
        sequence += 1;
        let responses = child.transact(&Request {
            sequence,
            at_ps: at_ps + offset,
            operation,
            args,
        })?;
        let completed = responses
            .last()
            .context("clock fixture completion")?
            .clone();
        ensure!(
            completed.kind == Kind::Complete && completed.at_ps == at_ps + offset,
            "clock fixture changed the requested timestamp"
        );
        observed.extend(responses);
        Ok(completed)
    };
    let mut drive = [0; 12];
    drive[1] = 10 << 32;
    drive[8] = 2;
    let first = command(1, Op::Drive, drive)?;
    ensure!(
        first.args[5] == 5001 + offset && first.args[6] == 1,
        "first owned clock edge"
    );
    if split {
        command(12001, Op::Advance, [0; 12])?;
    }
    drive[1] = 20 << 32;
    drive[4] = 1;
    drive[6] = 1;
    drive[7] = 1;
    drive[8] = 2 | 16 | 64;
    let boundary = command(15001, Op::Drive, drive)?;
    // Rising edges at5001 and15001 sampled the old input/reset0. The new
    // period preserves high level and schedules its falling edge at25001.
    ensure!(
        Logic::unpack(boundary.args[4], 19)?.known()? == 5,
        "same-time drive raced the old edge or its sequential sample"
    );
    ensure!(
        boundary.args[5] == 25001 + offset && boundary.args[6] == 1,
        "clock rephase did not preserve the actual old edge level"
    );
    let later = command(35001, Op::Advance, [0; 12])?;
    // The third rising edge samples both newly driven bits as1.
    ensure!(
        Logic::unpack(later.args[4], 19)?.known()? == 103,
        "new period/input/reset did not reach the next real rising edge"
    );
    ensure!(
        later.args[5] == 45001 + offset && later.args[6] == 1,
        "next clock phase"
    );
    command(35002, Op::Stop, [0; 12])?;
    let events: Vec<_> = observed
        .iter()
        .filter(|r| matches!(r.kind, Kind::Irq | Kind::Output))
        .map(|r| json!([r.at_ps, r.kind as u64, r.args[0]]))
        .collect();
    ensure!(
        events
            .iter()
            .any(|r| r[0] == 15001 + offset && r[1] == Kind::Output as u64),
        "coincident edge produced no actual output event"
    );
    if unknown_start {
        ensure!(
            observed
                .iter()
                .filter(|r| matches!(r.kind, Kind::Irq | Kind::Output))
                .all(|r| r.at_ps == 0 || r.at_ps >= 5001 + offset),
            "unknown or gated period created an early clock/output event"
        );
    }
    Ok(
        json!({"passed":true,"split_advance":split,"unknown_initial_period":unknown_start,"boundary_time_ps":15001+offset,
        "boundary_outputs":boundary.args[4],"later_outputs":later.args[4],
        "events":events,"event_comparison":"Compare actual time, kind and four-state value; command IDs differ because the split case has one additional Advance.",
        "input_policy":"Old clock events and sequential updates precede the future RW input batch; all new work settles before RO receipt.",
        "artificial_settling_delay_ps":0,"cpu_synchronization_complete":false}),
    )
}
