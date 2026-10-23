#!/usr/bin/env python3
"""Save one public artifact and a provenance manifest. Do not execute it."""
import argparse
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import time
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]

def utcnow():
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")

def artifact_directory(kind, artifact_id):
    """Keep stable workspace paths while using a registered archive drive for new large files."""
    directory = ROOT / "artifacts/files" / kind / artifact_id
    config_path = ROOT / "catalog/storage.json"
    if not directory.exists() and config_path.exists():
        config = json.loads(config_path.read_text())
        external = config.get("new_large_artifact_root")
        if external and kind in config.get("external_kinds", []):
            target = Path(external) / kind / artifact_id
            target.mkdir(parents=True, exist_ok=True)
            directory.parent.mkdir(parents=True, exist_ok=True)
            try:
                directory.symlink_to(target, target_is_directory=True)
            except FileExistsError:
                pass
    directory.mkdir(parents=True, exist_ok=True)
    return directory

def _fetch_unlocked(url, artifact_id, kind="document", title="", language="en", filename=None,
          max_bytes=1024**3, timeout=60, source_id=None, license_name="unknown", request_json=None):
    if not re.fullmatch(r"[a-z0-9][a-z0-9._-]*", artifact_id):
        raise ValueError("Use a lower-case artifact ID with no path separators.")
    manifest_path = ROOT / "artifacts/manifests" / (artifact_id + ".json")
    prior = {}
    if manifest_path.exists():
        prior = json.loads(manifest_path.read_text())
        if prior.get("status") == "downloaded":
            path = ROOT / prior["local_path"]
            if path.is_file() and path.stat().st_size == prior["size_bytes"]:
                return prior
    name = filename or urllib.parse.unquote(Path(urllib.parse.urlparse(url).path).name) or "index.html"
    name = re.sub(r"[^A-Za-z0-9._+-]", "_", name)[:180]
    directory = artifact_directory(kind, artifact_id)
    path = directory / name
    temp = path.with_suffix(path.suffix + ".part")
    # Keep discovery evidence when a queued or failed transfer is resumed.
    # Transport results and content checks must describe the new response.
    volatile = {"status", "local_path", "sha256", "size_bytes", "error", "http_status",
                "final_url", "content_type", "last_modified", "etag", "retrieved_at",
                "content_role", "payload_available", "lfs_object_sha256", "lfs_object_size",
                "content_note", "resolved_artifact_id"}
    record = {k: v for k, v in prior.items()
              if k not in volatile and not k.startswith("matches_")}
    record.update({"id": artifact_id, "url": url, "kind": kind, "title": title or artifact_id,
              "language": language, "source_id": source_id, "license": license_name,
              "retrieved_at": utcnow(), "status": "failed", "local_path": None,
              "sha256": None, "size_bytes": None, "executed": False})
    try:
        headers = {"User-Agent": "MiningDeviceCatalogue/1.0 (research archive; public files)", "Accept": "*/*"}
        body = None
        if request_json is not None:
            body = json.dumps(request_json).encode()
            headers["Content-Type"] = "application/json"
            record["request_json"] = request_json
            record["request_method"] = "POST"
        request = urllib.request.Request(url, data=body, headers=headers)
        with urllib.request.urlopen(request, timeout=timeout) as response:
            record.update(final_url=response.url, http_status=response.status,
                          content_type=response.headers.get("Content-Type"),
                          last_modified=response.headers.get("Last-Modified"),
                          etag=response.headers.get("ETag"))
            length = int(response.headers.get("Content-Length", "0"))
            record["http_content_length"] = length or None
            if length > max_bytes:
                raise ValueError(f"File size {length} exceeds limit {max_bytes}.")
            digest = hashlib.sha256()
            size = 0
            first = True
            with temp.open("wb") as handle:
                while True:
                    block = response.read(1024 * 1024)
                    if not block:
                        break
                    if first and kind in {"firmware", "source", "tool"} and (b"<!doctype html" in block[:500].lower() or b"<html" in block[:500].lower()):
                        raise ValueError("Server returned HTML instead of an artifact.")
                    first = False
                    size += len(block)
                    if size > max_bytes:
                        raise ValueError(f"File exceeds limit {max_bytes}.")
                    handle.write(block)
                    digest.update(block)
            if not size:
                raise ValueError("The response was empty.")
            if length and length != size:
                raise ValueError(f"Response size {size} does not match Content-Length {length}.")
            temp.replace(path)
            record.update(status="downloaded", local_path=str(path.relative_to(ROOT)),
                          size_bytes=size, sha256=digest.hexdigest())
            expected_size = record.get("expected_size", record.get("expected_size_bytes"))
            if isinstance(expected_size, int):
                record["matches_expected_size"] = size == expected_size
            upstream = record.get("upstream_digest")
            if isinstance(upstream, str) and re.fullmatch(r"sha256:[0-9a-fA-F]{64}", upstream):
                record["matches_upstream_digest"] = digest.hexdigest() == upstream[7:].lower()
            if size < 4096:
                pointer = path.read_bytes()
                match = re.fullmatch(rb'version https://git-lfs.github.com/spec/v1\r?\noid sha256:([0-9a-f]{64})\r?\nsize ([0-9]+)\r?\n?', pointer)
                if match:
                    record.update(content_role="git-lfs-pointer", payload_available=False,
                                  lfs_object_sha256=match[1].decode(), lfs_object_size=int(match[2]),
                                  content_note="This file is a Git LFS reference. It is not the firmware or document payload.")
    except Exception as exc:
        record["error"] = str(exc)
        if isinstance(exc, urllib.error.HTTPError):
            record["http_status"] = exc.code
            record["retry_after"] = exc.headers.get("Retry-After")
        temp.unlink(missing_ok=True)
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_manifest = manifest_path.with_suffix(".tmp")
    tmp_manifest.write_text(json.dumps(record, indent=2, ensure_ascii=False) + "\n")
    tmp_manifest.replace(manifest_path)
    return record

def fetch(url, artifact_id, kind="document", title="", language="en", filename=None,
          max_bytes=1024**3, timeout=60, source_id=None, license_name="unknown", request_json=None):
    if not re.fullmatch(r"[a-z0-9][a-z0-9._-]*", artifact_id):
        raise ValueError("Use a lower-case artifact ID with no path separators.")
    lockdir = ROOT / "work/download-locks"
    lockdir.mkdir(parents=True, exist_ok=True)
    with (lockdir / (artifact_id + ".lock")).open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        return _fetch_unlocked(url, artifact_id, kind, title, language, filename,
                               max_bytes, timeout, source_id, license_name, request_json)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("url")
    parser.add_argument("--id", required=True)
    parser.add_argument("--kind", choices=["document", "firmware", "source", "tool", "metadata", "web"], default="document")
    parser.add_argument("--title", default="")
    parser.add_argument("--language", default="en")
    parser.add_argument("--filename")
    parser.add_argument("--source-id")
    parser.add_argument("--license", default="unknown")
    parser.add_argument("--max-mib", type=int, default=1024)
    args = parser.parse_args()
    result = fetch(args.url, args.id, args.kind, args.title, args.language,
                   args.filename, args.max_mib*1024**2, source_id=args.source_id,
                   license_name=args.license)
    print(json.dumps(result, ensure_ascii=False))
    raise SystemExit(0 if result["status"] == "downloaded" else 1)

if __name__ == "__main__":
    main()
