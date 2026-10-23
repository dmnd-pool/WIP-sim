//! BIP310 initial negotiation. The server permits the BIP320 general-purpose
//! version bits. A mask remains fixed after activation or the first job.
//! Reference: archived BIPs commit927b6de9915c9262615a6399de51b200f81e5aa4.
use crate::Result;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const SERVER_MASK: u32 = 0x1fff_e000;

pub(super) fn configure(
    params: &Value,
    previous: Option<u32>,
    announced: bool,
) -> Result<(Value, Option<u32>)> {
    let args = params.as_array().ok_or("configure requires an array")?;
    if args.len() != 2 {
        return Err("configure requires extensions and parameters".into());
    }
    let extensions = args[0].as_array().ok_or("extension list is not an array")?;
    let parameters = args[1]
        .as_object()
        .ok_or("extension parameters are not a map")?;
    let mut seen = BTreeSet::new();
    let mut response = Map::new();
    let mut mask = previous;
    for extension in extensions {
        let name = extension.as_str().ok_or("extension code is not a string")?;
        if name.is_empty() || !seen.insert(name) {
            return Err("extension code is empty or duplicated".into());
        }
        if response.contains_key(name) {
            return Err("extension code collides with a response parameter".into());
        }
        if name != "version-rolling" {
            response.insert(name.into(), json!(false));
            continue;
        }
        let negotiated = (|| {
            if previous.is_some() || announced {
                return Err("version rolling is fixed after activation or the first job".to_owned());
            }
            parameters
                .get("version-rolling.min-bit-count")
                .and_then(Value::as_u64)
                .ok_or("minimum bit count must be a nonnegative integer")?;
            let requested = parameters
                .get("version-rolling.mask")
                .map(super::parse_u32_hex)
                .transpose()?
                .unwrap_or(u32::MAX);
            // The minimum is diagnostic. A smaller intersection must not
            // disconnect a client that can operate with fewer version bits.
            Ok(SERVER_MASK & requested)
        })();
        match negotiated {
            Ok(value) => {
                if response.contains_key("version-rolling.mask")
                    || extensions.iter().any(|name| name == "version-rolling.mask")
                {
                    return Err("extension code collides with the version mask parameter".into());
                }
                mask = Some(value);
                response.insert(name.into(), json!(true));
                response.insert("version-rolling.mask".into(), json!(format!("{value:08x}")));
            }
            Err(error) => {
                response.insert(name.into(), json!(error));
            }
        }
    }
    Ok((response.into(), mask))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negotiation_intersects_masks_without_expanding_permissions() {
        let (reply, mask) = configure(
            &json!([["version-rolling","future-extension"],{
                "version-rolling.mask":"F0002000","version-rolling.min-bit-count":16
            }]),
            None,
            false,
        )
        .unwrap();
        assert_eq!(mask, Some(0x1000_2000));
        assert_eq!(reply["version-rolling.mask"], "10002000");
        assert_eq!(reply["future-extension"], false);
        let (reply, mask) = configure(
            &json!([["version-rolling"],{"version-rolling.min-bit-count":0}]),
            None,
            false,
        )
        .unwrap();
        assert_eq!(mask, Some(SERVER_MASK));
        assert_eq!(reply["version-rolling"], true);
        for malformed in [json!("123"), json!("zzzzzzzz"), json!(4294967295u64)] {
            let (reply, mask) = configure(
                &json!([["version-rolling"],{
                    "version-rolling.mask":malformed,"version-rolling.min-bit-count":0
                }]),
                None,
                false,
            )
            .unwrap();
            assert!(mask.is_none() && reply["version-rolling"].is_string());
        }
        let (reply, mask) = configure(
            &json!([["version-rolling"],{"version-rolling.min-bit-count":0}]),
            Some(0x2000),
            true,
        )
        .unwrap();
        assert_eq!(mask, Some(0x2000));
        assert!(reply["version-rolling"].is_string());
    }
}
