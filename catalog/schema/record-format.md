# Catalogue record format

Use UTF-8 JSON Lines. Put one JSON object on each line.
Store research parts in `catalog/parts/<area>/`.
Use stable lower-case identifiers. Use hyphens between words.
Use ISO 8601 dates. Use `null` for an unknown value.
Do not infer compatibility from the model name or the processor alone.

## Entity

Required fields: `id`, `kind`, `name`, `manufacturer`, `source_ids`.

Kinds: `device`, `hashboard`, `control-board`, `chip`, `psu`, `firmware`,
`software`, `tool`, `protocol`, `platform`.

Recommended fields:

```json
{"id":"vendor-model","kind":"device","name":"Model","manufacturer":"Vendor","aliases":[],"algorithms":["SHA-256d"],"release_year":null,"release_date":null,"date_basis":"unknown","purpose":["commercial-mining"],"hardware":{},"software":{},"license":"unknown","source_ids":["source-id"],"artifact_ids":[],"notes":[],"evidence_status":"documented"}
```

Use `hardware` for processor, architecture, memory, board revision,
chip count, interfaces, and cooling. Give each known physical board
a separate entity when evidence identifies it. Use `software` for
versions, operating systems, boot methods, and installation limits.
The date basis must identify the exact event. Examples include `vendor-release`,
`published-source-release`, `earliest-known-model-design-file-commit`,
`document-date`, `secondary-report`, and `unknown`.
Preserve a more specific source description when available.
A document date or design-file commit does not prove a product shipment date.
An inherited repository ancestor does not establish the date of a later board design.

## Relationship

```json
{"id":"unique-edge-id","from_id":"firmware-id","to_id":"device-id","relation":"supports","status":"vendor-claimed","source_ids":["source-id"],"constraints":["Exact board revision is not specified."],"firmware_version":null,"hardware_revision":null,"notes":[]}
```

Relations: `supports`, `uses-chip`, `uses-control-board`, `uses-hashboard`,
`uses-psu`, `implements`, `derived-from`, `builds-for`, `requires`,
`supersedes`, `incompatible-with`, `communicates-with`, `variant-of`.

Statuses: `vendor-claimed`, `source-code`, `documented`, `community-reported`,
`inferred`, `tested`, `unknown`, `unsupported`, `archive-label-only`.
Reserve `tested` for a test that was actually done. Save its test evidence.
Compatibility is directed. It is not transitive.
`archive-label-only` preserves a filename or directory association.
It does not establish compatibility. The pair-check command returns `target-label-only` for this evidence alone.

## Source

```json
{"id":"source-id","url":"https://example.com/","title":"Title","publisher":"Publisher","source_type":"vendor","language":"en","retrieved_at":"2026-10-19T00:00:00Z","artifact_ids":[],"local_path":null,"notes":[]}
```

Example source types: `vendor`, `maintainer`, `standard`, `academic`, `community`,
`repair-vendor`, `distributor`, `archive`, `aggregator`.
Do not label a reseller as the equipment manufacturer.

## Research gap

```json
{"id":"gap-id","area":"Vendor or topic","description":"Missing evidence.","reason":"not-published","next_action":"Inspect the board label.","source_ids":[],"priority":"high"}
```

Each area contains `entities.jsonl`, `relationships.jsonl`, `sources.jsonl`,
and `gaps.jsonl`. Optional files can contain additional notes.
Download manifests are separate files in `artifacts/manifests/`.
Use `scripts/fetch_artifact.py` to collect a file with a checksum.
For a non-English source, save the original. Also save an English translation.
Record its scope. Do not call a translated extract a complete translation.

## Artifact and extracted component

A downloaded artifact has `id`, `url`, `kind`, `status`, `local_path`, `size_bytes`,
`sha256`, `retrieved_at`, and `executed` fields.
The normal statuses are `queued`, `downloaded`, and `failed`.
`superseded` records an acquisition attempt replaced by a specified alternative.
Read `acquisition_method` before you treat a record as a direct HTTP download.
An artifact can reuse a verified local object or identify a component of a parent file.

Keep these identities separate:

- `sha256`: measured identity of the saved bytes.
- `git_commit`: pinned source repository revision.
- `git_blob_sha1`: Git object identity, including the Git blob header.
- `publisher_sha256`, `publisher_md5`, or `upstream_digest`: a published identity claim.
- `parent_artifact_id` and `parent_sha256`: source file for an extracted component.
- `archive_member` or `byte_offset`: the component location inside its parent.

An extracted component must keep its own checksum and parent identity.
Use a separate record if its bytes change.
Do not assume that a readable installer script supplies the complete firmware source or an open-source licence.

`payload_available=false` means that the record does not supply a usable firmware payload.
A Git LFS reference can link to a separate `resolved_artifact_id`.
A damaged publisher file can remain saved for audit, with `content_validity` explaining the defect.

Some inventories call the published length `expected_size`; others use `expected_size_bytes`.
Keep the original field and compare it with measured `size_bytes`.
Neither a published length nor an HTTP Content-Length field replaces a content checksum.

## Import notes

The JSONL records preserve source-specific fields. Unknown fields are permitted.
Do not discard `constraints`, provenance, or unknown values when you normalize the data.
`purpose` can be text or a list of uses; treat a text value as a one-item list in a new application.
CSV object and array values are JSON strings.
The SQLite `json` column preserves the complete record.

## Evidence rules

- Open the cited source. Search results are only discovery aids.
- Record the exact model suffix and cooling form.
- Separate original firmware, source releases, and third-party firmware.
- Do not assume a public binary has a source licence.
- Record access failures. Do not bypass an access control.
- Do not run downloaded firmware or downloaded tools.
- Keep the manufacturer's measured values separate from simulation inputs.
