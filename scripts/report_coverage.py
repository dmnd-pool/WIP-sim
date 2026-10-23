#!/usr/bin/env python3
"""Build coverage and translation reports from the current evidence records."""
import collections,datetime,hashlib,json
from pathlib import Path
import catalog
R=catalog.ROOT

def main():
 data,supp=catalog.split_scope(catalog.load_parts());byid={e['id']:e for e in data['entities']}
 devices=[e for e in data['entities'] if e['kind']=='device'];edges=data['relationships'];artifacts=data['artifacts']
 lines=['# Coverage and limits','',f"Generated: {datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')}",'','This report describes this collection. It does not measure every file that exists on the Internet.','',
 '## Main scope','', 'Bitcoin ASIC devices, their firmware, and relevant support tools are in the main index.','Other mining algorithms remain outside the main scope.','','| Record type | Count |','| --- | ---: |']
 lines += [f'| {k} | {len(v):,} |' for k,v in data.items()]
 lines += ['','## Hardware evidence','', '| Field or relationship | Devices with evidence |','| --- | ---: |']
 metrics={'Device records':len(devices),'Release year with a stated basis':sum(e.get('release_year') is not None for e in devices)}
 for rel in ['uses-chip','uses-hashboard','uses-control-board','uses-psu']:
  ids={r['from_id'] for r in edges if r['relation']==rel}
  metrics[rel]=sum(e['id'] in ids for e in devices)
 lines += [f'| {k} | {v:,} |' for k,v in metrics.items()]
 lines += ['','These counts measure explicit graph links. A device can also have partial details in its `hardware` field.','A missing link is a research gap. It does not mean that the device lacks that component.','',
 '## Evidence status','', '| Relationship status | Count |','| --- | ---: |']
 lines += [f'| {k} | {v:,} |' for k,v in sorted(collections.Counter(r['status'] for r in edges).items())]
 lines += ['','No downloaded binary was booted or flashed as part of this collection.','Source claims and vendor claims are separate from local test evidence.','',
 '## File archive','', '| Kind | Downloaded records | Queued | Failed |','| --- | ---: | ---: | ---: |']
 for kind in sorted({a['kind'] for a in artifacts}):
  c=collections.Counter(a['status'] for a in artifacts if a['kind']==kind);lines.append(f"| {kind} | {c['downloaded']:,} | {c['queued']:,} | {c['failed']:,} |")
 saved=[a for a in artifacts if a['status']=='downloaded'];unique={a['sha256']:a for a in saved};logical=sum(a.get('size_bytes') or 0 for a in saved);unique_bytes=sum(a['size_bytes'] for a in unique.values())
 lines += ['',f'Downloaded records: {len(saved):,}. Unique SHA-256 values: {len(unique):,}.',f'Unique content size: {unique_bytes/2**30:.2f} GiB. Logical record size: {logical/2**30:.2f} GiB.','',
 'Logical size counts duplicate references. Unique content size counts each SHA-256 value once.','Neither value measures incomplete transfers or temporary working files.','']
 pointers=[a for a in artifacts if a.get('content_role')=='git-lfs-pointer'];lines += [f'Git LFS reference files: {len(pointers)}. These are not counted as firmware payloads.',f"References resolved to a saved object: {sum(bool(a.get('resolved_artifact_id')) for a in pointers)}.",'']
 check_fields=['matches_publisher_sha256','matches_all_publisher_sha256','matches_any_publisher_sha256','matches_any_publisher_size','matches_publisher_md5','vendor_md5_match','matches_lfs_oid','matches_lfs_size','matches_expected_size','matches_upstream_digest','matches_expected_size_bytes','upstream_integrity_match']
 mismatches=[a for a in artifacts if any(a.get(k) is False for k in check_fields)]
 lines += ['## Published checksum conflicts','', '| Artifact | Observed issue |','| --- | --- |']
 for a in mismatches:
  fields=[k for k in check_fields if a.get(k) is False]
  lines.append(f"| {a['id']} | {', '.join(fields)} |")
 if not mismatches:lines.append('| None recorded | No conflict was recorded by the collectors. |')
 lines += ['','A local hash check can pass while a publisher checksum conflict remains unresolved.','The local check proves that the saved file matches its manifest.','It does not resolve the publisher conflict.','',
 '## Access and knowledge gaps','', 'Read [the gap table](../catalog/csv/gaps.csv) for research actions.','Read [the artifact table](../catalog/csv/artifacts.csv) for exact HTTP failures and queued downloads.','Some unavailable firmware requires a vendor account, a support request, or a physical device.','No such access is implied by this collection.','',
 '## Enumeration boundaries','', 'The collection preserves vendor API responses, public directory lists, release feeds, and pinned repository trees.','These indexes establish what each source exposed at collection time.','They do not establish that all deleted releases, private repositories, or regional mirrors were found.','Discovery records remain leads until their claims are verified.','']
 anomalies=catalog.rows(R/'catalog/parts/open/esp-image-hash-anomalies.jsonl')
 lines += ['## Internal image checks','',f'ESP image hash anomalies recorded: {len(anomalies)}.','Read [the exact image anomaly records](../catalog/parts/open/esp-image-hash-anomalies.jsonl).','Some old factory bootloader images have a flash-configuration header that differs from the header covered by the stored appended hash.','The original bytes remain unchanged. This observation alone does not establish a boot result.','',
 'Read [the container check report](container-integrity.json) for ZIP CRC and gzip-stream checks on the root-collected subset.','Eleven Bitdeer updates have valid gzip streams followed by installer-script trailers.','The [Bitdeer layout index](../catalog/parts/emerging/bitdeer-image-layouts.jsonl) preserves their offsets and component hashes.','',
 'The [download failure index](download-failures.csv) separates rate limits, unavailable URLs, transfer errors, and saved alternatives.','Failure counts refer to records and attempts, not unique unavailable firmware releases.','A superseded attempt can remain indexed after another URL supplies the same payload.','']
 (R/'reports/coverage.md').write_text('\n'.join(lines))
 # Aggregate translation records. Preserve original coverage language and member references.
 tr=[]
 for p in list(sorted((R/'catalog/parts').glob('*/translations.jsonl'))) + [R/'translations/vendors/register.jsonl']:
  for i,row in enumerate(catalog.rows(p)):
   tr.append({**row,'register_path':str(p.relative_to(R)),'id':row.get('id') or 'translation-'+hashlib.sha256((str(p)+str(i)).encode()).hexdigest()[:16]})
 for p in (R/'translations/open').glob('*/translation.json'):
  row=json.loads(p.read_text());row['id']='translation-'+p.parent.name;row['register_path']=str(p.relative_to(R));row['translation_path']=row.get('english_path');tr.append(row)
 main_artifact_ids={a['id'] for a in artifacts}
 for row in tr:row['catalogue_scope']='bitcoin-or-shared' if main_artifact_ids.intersection(catalog.translation_artifact_ids(row)) else 'supplementary-or-unresolved'
 (R/'translations/register.jsonl').write_text(''.join(json.dumps(x,ensure_ascii=False)+'\n' for x in tr))
 tl=['# English translations','', 'Original files remain unchanged.','English companions are stored separately.','Each record states the source, method, review status, and translation coverage.','',
 'A full document translation and a translated appendix are different results.','An English vendor edition can differ from another language edition.','The register preserves that uncertainty.','',
 'Code, protocol bytes, filenames, register names, and binary payloads are not translated.','Text in images can require OCR and manual review.','OCR confidence is not a guarantee of correctness.','',
 '## Find a translation','', 'Use [register.jsonl](register.jsonl) for the complete machine-readable register.','Use the table below for direct file links.','Vendor English editions inside source archives have archive-member references in the register.','',
 '| Original artifact | English companion or archive member | Coverage |','| --- | --- | --- |']
 for row in tr:
  if row['catalogue_scope']!='bitcoin-or-shared':continue
  path=row.get('translation_path');label=f"[Open English text](../{path})" if path else '; '.join(row.get('translation_members',[]))
  tl.append('| '+ ' | '.join(str(v).replace('|','/').replace('\n',' ') for v in [', '.join(catalog.translation_artifact_ids(row)),label,row.get('scope','See register')])+' |')
 tl += ['','## Scanned Bitmain documents','', 'The [OCR register](../catalog/parts/bitmain/ocr-register.jsonl) records each page and its confidence.','It links extracted text and review information.','Translated OCR footers are listed above.','',
 '## Technical conflicts','', 'The Chinese and Russian Innosilicon error lists contain different service instructions.','Keep both English translations and their difference report.','Do not combine them into one unqualified repair rule.','',
 '## Remaining limits','', 'This is a register of completed companions, not a claim that every embedded string has been translated.','Review image labels, source-code comments, and newly collected documents when you select a device profile.','Supplier documents can have technical errors in the original language.','Translation does not remove those errors.','']
 (R/'translations/README.md').write_text('\n'.join(tl))
 print(json.dumps({'devices':len(devices),'artifacts':len(artifacts),'unique_content_gib':round(unique_bytes/2**30,2),'translation_records':len(tr)}))
if __name__=='__main__':main()
