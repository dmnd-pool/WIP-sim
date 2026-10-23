# Evidence and collection method

## Scope

The main scope is **Bitcoin ASIC mining hardware only**.
It includes current devices, historical devices, and relevant development projects.
Control-board tools, firmware tools, and Bitcoin mining protocols are supporting material.

The user confirmed this scope during collection.
Files for other algorithms, CPU miners, and FPGA miners had already been collected.
Those files remain in a separate supplementary index. They are excluded from the main catalogue.

The catalogue is a dated research snapshot.
It cannot prove that every public or private file has been found.
The discovery queue and gap table show unresolved work.

## Source order

Manufacturer documents and maintainer source code have priority.
Academic reports, repair documents, and community archives supply additional evidence.
Each source keeps its source type.
A distributor's website is not labelled as the manufacturer.

Search results identify possible sources. The collector then reads or downloads the source.
Some public pages use read APIs or JavaScript data.
The collector saves those responses when they contain the published download index.

No account messages, purchase requests, or support requests were sent.
No authentication barrier was bypassed.
An inaccessible URL remains an access failure in the manifest.

## Dates

The product release date, firmware release date, document date, and collection date are separate facts.
A firmware filename can provide a build-date clue.
It does not prove the product's release date.

The `date_basis` field identifies the date evidence.
An unknown release year remains `null`.
Conflicting date evidence remains visible in the notes.
The discovery queue can contain reported dates that are not verified.

## Compatibility

Each compatibility record is directed and has a source.
Its constraints can name board revisions, firmware versions, cooling types, or installation limits.
Compatibility is not transitive.
Support for a chip does not prove support for every hashboard that contains it.

The `tested` status is reserved for tests performed for this collection.
A vendor's test report remains a vendor claim in this catalogue.
A source-code backend can be incomplete or disabled.
Its presence alone does not prove successful mining.

## Download identity

Each download manifest records its URL, access time, size, and SHA-256 checksum.
Source archives use a commit ID when available.
The mirror collector also checks Git blob identity against the saved repository tree.
Auradine files have a separate check against the publisher's SHA-256 value.

A checksum confirms file identity. It does not establish that a file is authentic or suitable for a device.
Public access does not establish an open-source licence.
A source archive can omit submodules, toolchains, proprietary components, and build dependencies.

The original files retain their bytes.
The collection does not execute downloaded firmware or downloaded management tools.
Static inspection uses the host's file and archive readers.

## Archive labels

Archive paths are retained as supplied.
They are search metadata, not technical specifications.
For example, some indexes place Antminer X3 under RandomX.
That directory label must not establish the model's algorithm.

## Research limits

Some vendors publish only update packages or management applications.
Some boot images require a support request.
Some current source trees contain development targets with no mining test evidence.
Some discontinued vendors no longer host files.

The 2026 firmware study describes a large corpus, but its authors do not publish the binary dataset.
Its absence is recorded as a gap. [Study, Appendix A](https://arxiv.org/html/2605.03770v1#A1)

Hardware-register maps, signed boot components, calibration data, and physical traces remain important gaps for a universal simulator.
