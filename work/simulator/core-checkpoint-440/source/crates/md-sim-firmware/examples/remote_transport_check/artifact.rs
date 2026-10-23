//! Actual lost acknowledgements, followed by the production transfer path.
use super::*;
use md_sim_coordinator::{executor::transfer_object, Executor, WorkerExecutor};

pub fn check(events: &mut Vec<Value>) -> Result<()> {
    let store = Store::open(Path::new("/artifact-recovery-store"), 1 << 20)?;
    let mut cases = Vec::new();
    for operation in ["chunk", "finish"] {
        let mut fresh = vec![0; ssh_artifact_ack_loss::SIZE as usize];
        fs::File::open("/dev/urandom")?.read_exact(&mut fresh)?;
        let source = Path::new("/work").join(format!("artifact-{operation}.bin"));
        fs::write(&source, &fresh)?;
        let object = hash_file(&source)?;
        store.import(&source, &object)?;
        let mut normal = WorkerExecutor {
            transport: SshTransport::new_streaming(config())?,
        };
        let before = serde_json::to_value(normal.identity()?)?;
        let mut fault = config();
        fault.remote_relay = format!("/drop-{operation}-ack-relay");
        let mut interrupted = SshTransport::new_streaming(fault)?;
        let error = transfer_object(&mut interrupted, &store, &object, &AtomicBool::new(false))
            .expect_err("artifact write with a withheld reply succeeded");
        let error = format!("{error:#}");
        let withheld = ssh_artifact_ack_loss::observation(&error)?;
        let retries = interrupted.reconnect_count();
        drop(interrupted);
        let resumed = normal.transport.request(&Request::Begin {
            object: object.clone(),
        })?;
        // Re-enter the real coordinator path. Begin supplies the destination
        // offset. Finish rechecks the complete hash, including after a lost
        // publication acknowledgement. No mutation is blindly retried.
        transfer_object(
            &mut normal.transport,
            &store,
            &object,
            &AtomicBool::new(false),
        )?;
        let after = serde_json::to_value(normal.identity()?)?;
        let mut retrieved = Vec::new();
        while (retrieved.len() as u64) < object.size {
            let response = normal.transport.request(&Request::Get {
                object: object.clone(),
                offset: retrieved.len() as u64,
                count: MAX_CHUNK,
            })?;
            let bytes = hex::decode(
                response["hex"]
                    .as_str()
                    .context("retrieved artifact bytes absent")?,
            )?;
            ensure!(
                !bytes.is_empty()
                    && bytes.len() <= MAX_CHUNK
                    && retrieved.len() + bytes.len() <= fresh.len(),
                "retrieved artifact chunk bound"
            );
            retrieved.extend(bytes);
        }
        ensure!(
            retrieved == fresh,
            "resumed artifact differs from fresh input"
        );
        let verified = normal.transport.request(&Request::Finish {
            object: object.clone(),
        })?;
        cases.push(
            json!({"operation":operation,"object":object,"worker_before":before,
            "worker_after":after,"interrupted_error":error,"automatic_mutation_reconnects":retries,
            "withheld_reply":withheld,"resume_begin":resumed,"destination_finish":verified,
            "retrieved":{"sha256":hex::encode(Sha256::digest(&retrieved)),"size":retrieved.len()},
            "same_fresh_bytes":true,"production_transfer_resumed":true}),
        );
    }
    let proof = json!({"cases":cases,"firmware_gate":false});
    isolation::save_json(Path::new("/work/artifact-ack-loss.json"), &proof)?;
    dashboard_transport_report::verify_artifact_recovery(&proof)?;
    events.push(json!({"actual_artifact_reply_loss":proof}));
    Ok(())
}
