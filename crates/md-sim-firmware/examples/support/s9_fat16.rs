//! Bounded read-only FAT16 allocation inspection for a root 8.3 file.
//! This is storage evidence, not a BootROM implementation or EOF policy.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::prepare::hash_bytes;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn bytes(b: &[u8], at: usize, n: usize) -> Result<&[u8]> {
    b.get(at..at.checked_add(n).context("byte range overflow")?)
        .context("byte range exceeds image")
}
fn u16le(b: &[u8], at: usize) -> Result<usize> {
    Ok(u16::from_le_bytes(bytes(b, at, 2)?.try_into()?) as usize)
}
fn u32le(b: &[u8], at: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(bytes(b, at, 4)?.try_into()?) as usize)
}
fn object(b: &[u8]) -> Value {
    json!({"sha256":hash_bytes(b),"size":b.len()})
}

pub struct Allocation {
    pub logical: Vec<u8>,
    pub final_sector: Vec<u8>,
    pub report: Value,
}

pub fn inspect(image: &[u8], name: &[u8; 11]) -> Result<Allocation> {
    ensure!(image.len() <= 256 << 20, "image bound");
    ensure!(bytes(image, 510, 2)? == [0x55, 0xaa], "MBR signature");
    let entry = bytes(image, 446, 16)?;
    ensure!(
        matches!(entry[4], 0x04 | 0x06 | 0x0b | 0x0c | 0x0e),
        "first partition type unsupported"
    );
    let start = u32le(entry, 8)?
        .checked_mul(512)
        .context("partition start overflow")?;
    let size = u32le(entry, 12)?
        .checked_mul(512)
        .context("partition size overflow")?;
    ensure!(start >= 512 && size > 0, "partition bounds");
    let part = bytes(image, start, size)?;
    ensure!(bytes(part, 510, 2)? == [0x55, 0xaa], "FAT boot signature");
    let sector = u16le(part, 11)?;
    ensure!(sector == 512, "only 512-byte sectors are implemented");
    let per_cluster = part[13] as usize;
    ensure!(
        per_cluster.is_power_of_two() && per_cluster <= 128,
        "cluster geometry"
    );
    let reserved = u16le(part, 14)?;
    let copies = part[16] as usize;
    let roots = u16le(part, 17)?;
    let fat_sectors = u16le(part, 22)?;
    let short_total = u16le(part, 19)?;
    let long_total = u32le(part, 32)?;
    ensure!(
        short_total == 0 || long_total == 0,
        "conflicting total-sector fields"
    );
    let total = if short_total == 0 {
        long_total
    } else {
        short_total
    };
    ensure!(
        reserved > 0 && copies == 2 && roots > 0 && fat_sectors > 0,
        "FAT16 geometry"
    );
    ensure!(
        total.checked_mul(sector).context("volume overflow")? <= size,
        "volume exceeds partition"
    );
    let root_sectors = (roots * 32).div_ceil(sector);
    let fat_at = reserved * sector;
    let fat_size = fat_sectors * sector;
    let root_at = fat_at + copies * fat_size;
    let data_at = root_at + root_sectors * sector;
    ensure!(data_at < total * sector, "empty data region");
    let clusters = (total - data_at / sector) / per_cluster;
    ensure!(
        (4085..65525).contains(&clusters),
        "BPB does not describe FAT16"
    );
    ensure!((clusters + 2) * 2 <= fat_size, "FAT too short for volume");
    let fat = bytes(part, fat_at, fat_size)?;
    ensure!(
        fat == bytes(part, fat_at + fat_size, fat_size)?,
        "FAT copies disagree"
    );
    ensure!(
        u16le(fat, 0)? == 0xff00 | usize::from(part[21]) && u16le(fat, 2)? >= 0xfff8,
        "reserved FAT entries"
    );
    let mut selected = None;
    for (index, record) in bytes(part, root_at, roots * 32)?
        .chunks_exact(32)
        .enumerate()
    {
        if record[0] == 0 {
            break;
        }
        if record[0] == 0xe5 || record[11] == 0x0f {
            continue;
        }
        if &record[..11] != name {
            continue;
        }
        ensure!(
            record[11] & 0x18 == 0 && u16le(record, 20)? == 0,
            "root entry is not a FAT16 regular file"
        );
        ensure!(selected.is_none(), "duplicate root file name");
        selected = Some((root_at + index * 32, u16le(record, 26)?, u32le(record, 28)?));
    }
    let (directory_at, mut cluster, length) = selected.context("root file absent")?;
    ensure!(length > 0 && length <= 16 << 20, "file size bound");
    let cluster_size = per_cluster * sector;
    let expected_clusters = length.div_ceil(cluster_size);
    let mut seen = BTreeSet::new();
    let mut chain = Vec::new();
    let mut allocated = Vec::new();
    let mut spans = Vec::new();
    let end_marker;
    loop {
        ensure!(
            (2..clusters + 2).contains(&cluster) && cluster < 0xfff0,
            "invalid data cluster"
        );
        ensure!(seen.insert(cluster), "FAT chain loop");
        ensure!(
            chain.len() < expected_clusters,
            "chain exceeds logical file allocation"
        );
        let at = data_at + (cluster - 2) * cluster_size;
        let contents = bytes(part, at, cluster_size)?;
        spans.push(json!({"cluster":cluster,"image_offset":start+at,"object":object(contents)}));
        allocated.extend_from_slice(contents);
        chain.push(cluster);
        let next = u16le(fat, cluster * 2)?;
        if next >= 0xfff8 {
            end_marker = next;
            break;
        }
        cluster = next;
    }
    ensure!(chain.len() == expected_clusters, "short FAT chain");
    let logical = allocated[..length].to_vec();
    let slack = &allocated[length..];
    let last = *chain.last().context("empty chain")?;
    let final_sector_at =
        data_at + (last - 2) * cluster_size + ((length - 1) % cluster_size / sector) * sector;
    let final_sector = bytes(part, final_sector_at, sector)?.to_vec();
    let adjacent = data_at + (last - 1) * cluster_size;
    let adjacent_count = 4096.min(total * sector - adjacent);
    let next = bytes(part, adjacent, adjacent_count)?;
    let report = json!({"schema":"md-sim-fat16-allocation/1","partition":{"image_offset":start,"bytes":size,"mbr_type":entry[4],"mbr_label_conflicts_with_bpb_fat16":matches!(entry[4],0x0b|0x0c),"type_policy":"Admit a FAT partition label, then derive FAT16 width from BPB cluster count. A FAT32 MBR label is retained as a conflict, not compatibility proof."},
        "geometry":{"bytes_per_sector":sector,"sectors_per_cluster":per_cluster,"reserved_sectors":reserved,"fat_copies":copies,"fat_sectors":fat_sectors,"root_entries":roots,"total_sectors":total,"data_clusters":clusters},
        "fat":{"object":object(fat),"copies_equal":true},"directory":{"image_offset":start+directory_at,"entry":hex::encode(bytes(part,directory_at,32)?)},
        "file":object(&logical),"allocated":object(&allocated),"chain":chain,"spans":spans,"end_marker":end_marker,
        "final_logical_sector":{"image_offset":start+final_sector_at,"object":object(&final_sector)},
        "slack":{"object":object(slack),"nonzero_bytes":slack.iter().filter(|&&x|x!=0).count()},
        "adjacent_physical_bytes":{"image_offset":start+adjacent,"object":object(next),"nonzero_bytes":next.iter().filter(|&&x|x!=0).count(),"belongs_to_file":false},
        "scope":"Logical bytes follow the FAT chain. Slack and physically adjacent bytes are evidence only; no BootROM read policy is inferred."});
    Ok(Allocation {
        logical,
        final_sector,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put16(b: &mut [u8], at: usize, x: u16) {
        b[at..at + 2].copy_from_slice(&x.to_le_bytes());
    }
    fn put32(b: &mut [u8], at: usize, x: u32) {
        b[at..at + 4].copy_from_slice(&x.to_le_bytes());
    }
    fn fixture() -> Vec<u8> {
        let mut b = vec![0u8; 8193 * 512];
        b[510..512].copy_from_slice(&[0x55, 0xaa]);
        b[450] = 6;
        put32(&mut b, 454, 1);
        put32(&mut b, 458, 8192);
        let p = &mut b[512..];
        p[510..512].copy_from_slice(&[0x55, 0xaa]);
        put16(p, 11, 512);
        p[13] = 1;
        put16(p, 14, 1);
        p[16] = 2;
        put16(p, 17, 16);
        put16(p, 19, 8192);
        p[21] = 0xf8;
        put16(p, 22, 32);
        for base in [512, 33 * 512] {
            put16(p, base, 0xfff8);
            put16(p, base + 2, 0xffff);
            put16(p, base + 4, 4);
            put16(p, base + 8, 0xffff);
        }
        let root = 65 * 512;
        p[root..root + 11].copy_from_slice(b"BOOT    BIN");
        p[root + 11] = 32;
        put16(p, root + 26, 2);
        put32(p, root + 28, 600);
        p[66 * 512..67 * 512].fill(0x11);
        p[67 * 512..68 * 512].fill(0xee);
        p[68 * 512..69 * 512].fill(0x22);
        b
    }
    #[test]
    fn follows_fragmented_chain_and_keeps_slack_separate() {
        let a = inspect(&fixture(), b"BOOT    BIN").unwrap();
        assert_eq!(a.logical, [vec![0x11; 512], vec![0x22; 88]].concat());
        assert_eq!(a.report["chain"], json!([2, 4]));
        assert_eq!(a.report["slack"]["object"]["size"], 424);
        assert_eq!(
            a.report["adjacent_physical_bytes"]["belongs_to_file"],
            false
        );
    }
    #[test]
    fn rejects_mirror_loop_short_chain_duplicate_and_truncation() {
        let mut b = fixture();
        b[1028] ^= 1;
        assert!(inspect(&b, b"BOOT    BIN").is_err());
        for next in [2, 0, 0xfff7, 0xffff] {
            let mut b = fixture();
            for base in [1024, 34 * 512] {
                put16(&mut b, base + 4, next);
            }
            assert!(inspect(&b, b"BOOT    BIN").is_err(), "{next:x}");
        }
        let mut b = fixture();
        let at = 66 * 512;
        b.copy_within(at..at + 32, at + 32);
        assert!(inspect(&b, b"BOOT    BIN").is_err());
        let mut b = fixture();
        b.pop();
        assert!(inspect(&b, b"BOOT    BIN").is_err());
    }
    #[test]
    fn records_mbr_label_conflict_without_changing_bpb_interpretation() {
        let mut b = fixture();
        b[450] = 0x0c;
        let a = inspect(&b, b"BOOT    BIN").unwrap();
        assert_eq!(
            a.report["partition"]["mbr_label_conflicts_with_bpb_fat16"],
            true
        );
        assert_eq!(a.logical.len(), 600);
    }
    #[test]
    fn rejects_wrong_geometry_and_missing_name() {
        let mut b = fixture();
        b[512 + 13] = 3;
        assert!(inspect(&b, b"BOOT    BIN").is_err());
        assert!(inspect(&fixture(), b"OTHER   BIN").is_err());
    }
}
