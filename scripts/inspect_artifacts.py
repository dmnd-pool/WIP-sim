#!/usr/bin/env python3
"""Read artifact types and archive member names. Do not execute artifact code."""
import concurrent.futures
import fcntl
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import zipfile
from html.parser import HTMLParser
from fetch_artifact import ROOT

class VisibleText(HTMLParser):
    def __init__(self):
        super().__init__(); self.hidden = 0; self.text = []
    def handle_starttag(self, tag, attrs):
        if tag in {"script", "style", "template", "svg"}: self.hidden += 1
        if tag in {"p", "div", "li", "h1", "h2", "h3", "h4", "tr", "pre", "br"}: self.text.append("\n")
    def handle_endtag(self, tag):
        if tag in {"script", "style", "template", "svg"}: self.hidden = max(0, self.hidden-1)
        if tag in {"p", "div", "li", "h1", "h2", "h3", "h4", "tr", "pre"}: self.text.append("\n")
    def handle_data(self, data):
        if not self.hidden: self.text.append(data)

def inspect(manifest):
    path = ROOT / manifest["local_path"]
    result = {"artifact_id": manifest["id"], "sha256": manifest["sha256"], "executed": False,
              "scope": "Outer file type and archive member list. No boot test. No recursive filesystem extraction."}
    try:
        result["file_type"] = subprocess.run(["file", "-b", "--", str(path)], capture_output=True, text=True, timeout=15).stdout.strip()
        with path.open("rb") as handle:
            head = handle.read(512)
        result["header_hex"] = head[:32].hex()
        if manifest.get("kind") == "web" and path.stat().st_size < 30*1024**2:
            parser = VisibleText(); parser.feed(path.read_text(errors="replace"))
            lines = [" ".join(line.split()) for line in "".join(parser.text).splitlines()]
            textpath = ROOT / "sources/web-text" / (manifest["id"] + ".txt")
            textpath.parent.mkdir(parents=True, exist_ok=True)
            textpath.write_text("\n".join(line for line in lines if line) + "\n")
            result["text_path"] = str(textpath.relative_to(ROOT))
            result["text_sha256"] = hashlib.sha256(textpath.read_bytes()).hexdigest()
            result["text_extraction"] = "Visible text from archived HTML. Dynamic content and image text can be absent."
        if head.startswith(b"version https://git-lfs.github.com/spec/v1"):
            result["content_role"] = "git-lfs-pointer"
            result["payload_available"] = False
            result["inspection_note"] = "Git LFS reference only. Do not boot, flash, or count this as a firmware image."
        if head[:4] == b"\x7fELF":
            order = "little" if head[5] == 1 else "big"
            result["elf_machine"] = int.from_bytes(head[18:20], order)
            result["elf_class"] = 32 if head[4] == 1 else 64
            result["elf_byte_order"] = order
        entries = []
        archive_kind = None
        if head.startswith(b"PK\x03\x04"):
            with zipfile.ZipFile(path) as archive:
                entries = [{"path": m.filename, "size": m.file_size} for m in archive.infolist()[:50000]]
                result["archive_truncated"] = len(archive.infolist()) > 50000
            archive_kind = "zip"
        elif head.startswith(b"\x1f\x8b") or path.name.endswith((".tar.gz", ".tgz", ".tar", ".tar.xz", ".tar.bz2")):
            try:
                with tarfile.open(path, "r|*") as archive:
                    for index, member in enumerate(archive):
                        if index >= 50000:
                            result["archive_truncated"] = True
                            break
                        entries.append({"path": member.name, "size": member.size, "type": member.type.decode(errors="replace")})
                archive_kind = "tar"
            except tarfile.ReadError:
                result["archive_note"] = "The name or compression header suggests an archive, but the TAR parser did not accept it. A compressed non-TAR file is also possible."
        if archive_kind:
            result["archive_kind"] = archive_kind
            result["listed_members"] = len(entries)
            result.setdefault("archive_truncated", False)
            names = ROOT / "reports/archive-members" / (manifest["id"] + ".jsonl")
            names.parent.mkdir(parents=True, exist_ok=True)
            names.write_text("".join(json.dumps(entry, ensure_ascii=False) + "\n" for entry in entries))
            result["member_index"] = str(names.relative_to(ROOT))
        if head.startswith(b"%PDF"):
            textpath = ROOT / "sources/document-text" / (manifest["id"] + ".txt")
            textpath.parent.mkdir(parents=True, exist_ok=True)
            process = subprocess.run(["pdftotext", "-layout", str(path), str(textpath)], capture_output=True, text=True, timeout=90)
            if process.returncode == 0:
                result["text_path"] = str(textpath.relative_to(ROOT))
                result["text_sha256"] = hashlib.sha256(textpath.read_bytes()).hexdigest()
                result["text_extraction"] = "PDF text layer. Text in images can be absent."
            else:
                result["text_extraction_error"] = process.stderr[:300]
    except Exception as exc:
        result["inspection_error"] = str(exc)
    return result

def main():
    out = ROOT / "reports/artifact-inspection.jsonl"
    existing = {x["artifact_id"]: x for x in (json.loads(line) for line in out.read_text().splitlines())} if out.exists() else {}
    manifests = [json.loads(p.read_text()) for p in (ROOT / "artifacts/manifests").glob("*.json")]
    def needs_inspection(m):
        previous = existing.get(m["id"], {})
        if previous.get("sha256") != m["sha256"]: return True
        if m.get("kind") == "web" and not previous.get("text_path"): return True
        return previous.get("header_hex", "").startswith("1f8b") and not previous.get("archive_kind") and not previous.get("archive_note")
    pending = [m for m in manifests if m.get("status") == "downloaded" and m.get("kind") in {"source", "firmware", "tool", "document", "web"}
               and needs_inspection(m)]
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        for result in pool.map(inspect, pending):
            existing[result["artifact_id"]] = result
            print(result["artifact_id"], result.get("file_type", result.get("inspection_error")), flush=True)
    out.write_text("".join(json.dumps(v, ensure_ascii=False) + "\n" for v in existing.values()))

if __name__ == "__main__":
    lock = ROOT / "work/artifact-inspection.lock"
    lock.parent.mkdir(parents=True, exist_ok=True)
    with lock.open("a") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        main()
