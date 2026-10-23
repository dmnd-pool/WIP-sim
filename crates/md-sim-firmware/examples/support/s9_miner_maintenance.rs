//! Use the archived native stop/start API. Pool observations only synchronize
//! this test; they do not replace the independent hardware and header audit.
use super::{advancing_uptime, Client, UPTIME};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{ErrorKind, Read},
    net::TcpStream,
    path::Path,
    time::{Duration, Instant},
};

const STOP: &str = "mutation { bosminer { stop { __typename ... on BosminerError { message } } } }";
const START: &str =
    "mutation { bosminer { start { __typename ... on BosminerError { message } } } }";
const TRACE_LIMIT: u64 = 512 * 1024;

fn accepted_jobs(bytes: &[u8]) -> Result<BTreeSet<String>> {
    let mut requests = BTreeMap::new();
    let mut advertised = BTreeSet::new();
    let mut accepted = BTreeSet::new();
    // The writer can be between its write and newline. Only complete records
    // are observations. A malformed complete record remains a hard failure.
    for line in bytes
        .split_inclusive(|b| *b == b'\n')
        .filter(|s| s.ends_with(b"\n"))
    {
        let event: Value = serde_json::from_slice(line)?;
        let message = &event["message"];
        match event["direction"].as_str() {
            Some("firmware-to-pool") if message["method"] == "mining.submit" => {
                let id = message["id"].as_u64().context("submission ID absent")?;
                let job = message["params"][1]
                    .as_str()
                    .context("submission job absent")?;
                ensure!(
                    requests.insert(id, job.to_owned()).is_none(),
                    "pending submission ID reused"
                );
            }
            Some("pool-to-firmware") => {
                if message["method"] == "mining.notify" {
                    advertised.insert(
                        message["params"][0]
                            .as_str()
                            .context("notification job absent")?
                            .to_owned(),
                    );
                }
                if let Some(job) = message["id"].as_u64().and_then(|id| requests.remove(&id)) {
                    if message["result"] == true && message["error"].is_null() {
                        ensure!(
                            advertised.contains(&job),
                            "accepted submission has no notification"
                        );
                        accepted.insert(job);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(accepted)
}

fn progress(root: &Path) -> Result<BTreeMap<usize, BTreeSet<String>>> {
    let mut sessions = BTreeMap::new();
    for id in 0..16 {
        let mut file = match File::open(root.join(format!("session-{id}.jsonl"))) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        (&mut file).take(TRACE_LIMIT + 1).read_to_end(&mut bytes)?;
        ensure!(bytes.len() as u64 <= TRACE_LIMIT, "maintenance trace bound");
        sessions.insert(id, accepted_jobs(&bytes)?);
    }
    Ok(sessions)
}

fn wait_work(client: &Client, after: Option<usize>) -> Result<BTreeMap<usize, BTreeSet<String>>> {
    let deadline = client
        .deadline
        .min(Instant::now() + Duration::from_secs(75));
    loop {
        let observed = progress(Path::new("/work"))?;
        if observed
            .iter()
            .any(|(id, jobs)| after.is_none_or(|previous| *id > previous) && jobs.len() >= 2)
        {
            return Ok(observed);
        }
        ensure!(
            Instant::now() < deadline,
            "native miner did not submit accepted work for two jobs within maintenance budget"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_port(client: &Client, listening: bool) -> Result<Value> {
    let started = Instant::now();
    let deadline = client.deadline.min(started + Duration::from_secs(30));
    let address = "10.0.2.15:4028".parse()?;
    loop {
        let result = TcpStream::connect_timeout(&address, Duration::from_millis(200));
        let refused = result
            .as_ref()
            .is_err_and(|e| e.kind() == ErrorKind::ConnectionRefused);
        if (listening && result.is_ok()) || (!listening && refused) {
            return Ok(json!({"endpoint":"10.0.2.15:4028","listening":listening,
                "connection_refused":refused,"elapsed_milliseconds":started.elapsed().as_millis()}));
        }
        ensure!(
            Instant::now() < deadline,
            "native miner API did not reach expected listening state"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn require_control(value: &Value, operation: &str) -> Result<()> {
    ensure!(
        value["bosminer"][operation]["__typename"] == "VoidResult",
        "native miner {operation} failed: {value}"
    );
    Ok(())
}

pub(super) fn stop(client: &mut Client) -> Result<Value> {
    let jobs = wait_work(client, None)?;
    let before = client.graphql(UPTIME, json!({}))?;
    let response = client.graphql(STOP, json!({}))?;
    require_control(&response, "stop")?;
    let port = wait_port(client, false)?;
    std::thread::sleep(Duration::from_secs(1));
    let after = client.graphql(UPTIME, json!({}))?;
    advancing_uptime(&before, &after)?;
    let result = json!({"operation":"stop","response":response,"port":port,
        "observed_jobs":jobs,"last_session":jobs.keys().last(),"before":before,"after":after});
    println!("MD_SIM_NATIVE_MINER_CONTROL {result}");
    Ok(result)
}

pub(super) fn start(client: &mut Client, stopped: &Value) -> Result<Value> {
    let last = stopped["last_session"]
        .as_u64()
        .context("maintenance session checkpoint absent")?;
    let response = client.graphql(START, json!({}))?;
    require_control(&response, "start")?;
    let port = wait_port(client, true)?;
    let jobs = wait_work(client, Some(usize::try_from(last)?))?;
    let result = json!({"operation":"start","response":response,"port":port,
        "observed_jobs":jobs,"after_session":last,
        "scope":"Native control and live pool progress. Full independent header and hardware audit is separate."});
    println!("MD_SIM_NATIVE_MINER_CONTROL {result}");
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn records(events: &[(bool, Value)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (server, message) in events {
            bytes.extend(serde_json::to_vec(&json!({"direction":if *server {"pool-to-firmware"} else {"firmware-to-pool"},"message":message})).unwrap());
            bytes.push(b'\n');
        }
        bytes
    }
    #[test]
    fn progress_requires_correlated_submissions_and_distinct_announced_jobs() {
        let mut events = vec![(true, json!({"id":1,"result":true,"error":null}))];
        assert!(accepted_jobs(&records(&events)).unwrap().is_empty());
        events.extend([
            (true, json!({"method":"mining.notify","params":["fresh-a"]})),
            (
                false,
                json!({"id":2,"method":"mining.submit","params":["simulator","fresh-a"]}),
            ),
            (
                true,
                json!({"id":2,"result":false,"error":[23,"LowDifficulty",null]}),
            ),
        ]);
        assert!(accepted_jobs(&records(&events)).unwrap().is_empty());
        for id in [3, 4] {
            events.extend([
                (
                    false,
                    json!({"id":id,"method":"mining.submit","params":["simulator","fresh-a"]}),
                ),
                (true, json!({"id":id,"result":true,"error":null})),
            ]);
        }
        assert_eq!(accepted_jobs(&records(&events)).unwrap().len(), 1);
        events.extend([
            (true, json!({"method":"mining.notify","params":["fresh-b"]})),
            (
                false,
                json!({"id":5,"method":"mining.submit","params":["simulator","fresh-b"]}),
            ),
            (true, json!({"id":5,"result":true,"error":null})),
        ]);
        let bytes = records(&events);
        assert_eq!(accepted_jobs(&bytes).unwrap().len(), 2);
        assert_eq!(accepted_jobs(&bytes[..bytes.len() - 1]).unwrap().len(), 1);
        assert!(accepted_jobs(b"{invalid}\n").is_err());
        assert!(accepted_jobs(&records(&[
            (
                false,
                json!({"id":1,"method":"mining.submit","params":["simulator","unknown"]})
            ),
            (true, json!({"id":1,"result":true,"error":null}))
        ]))
        .is_err());
    }
    #[test]
    fn control_errors_do_not_count_as_success() {
        require_control(
            &json!({"bosminer":{"stop":{"__typename":"VoidResult"}}}),
            "stop",
        )
        .unwrap();
        assert!(require_control(
            &json!({"bosminer":{"stop":{"__typename":"BosminerError","message":"failed"}}}),
            "stop"
        )
        .is_err());
        assert!(require_control(
            &json!({"bosminer":{"start":{"__typename":"VoidResult"}}}),
            "stop"
        )
        .is_err());
    }
}
