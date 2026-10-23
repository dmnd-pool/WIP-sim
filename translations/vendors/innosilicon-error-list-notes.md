# Innosilicon error lists: English translations

The two CSV files are separate complete translations of the archived Chinese and Russian lists. They retain all error codes, error names, fields, row order, blank fields, and final notes. The original files remain unchanged. The translations were made with AI and have not had a human review.

- [Chinese list in English](innosilicon-error-list-cn-en.csv): original artifact `mirror-033dfdf8e8a0cc11`, `ErrorListCN.csv`.
- [Russian list in English](innosilicon-error-list-ru-en.csv): original artifact `mirror-ea6276bbacdb9b51`, `ErrorListRU.csv`.

The files come from a public community firmware archive. The exact author, issue date, applicable models, and firmware versions are not established. Use them as evidence of diagnostic vocabulary and possible error states. Do not infer that each code applies to every Innosilicon miner.

The documents disagree. Preserve these differences in a simulator profile until a device trace or a primary manual resolves them.

| Code | Chinese document | Russian document |
|---|---|---|
| 21 | Error information gives detected hashboard numbers. | Error information gives detected power-board numbers. |
| 31 | No action is required. | Send for repair. |
| 35 | Step 1 says no action if hash rate is normal and no board goes offline. | Step 1 says restart under the same condition. |
| 36 | Description refers to good chip cores. Step 1 says no action if operation is normal. | Description refers to good chips. Step 1 says restart if operation is normal. |
| 37 | After more than 10 affected units, contact software R&D for a joint resolution. | After more than 10 affected units, contact repair service. |
| 38, 39 | No action is currently required. | Send for repair. |
| Final notes | Three notes about power cycling after each step, returning unhelpful replacement parts, and labelling returned boards. | No final notes. |

Several Russian parenthetical repair terms are unclear. The English translation preserves their literal wording, such as “rework the puzzle board” and “join the board”. These are not validated repair procedures. The Chinese term `拼板` is translated as “board assembly”; the exact service process is not established. Both files say “40 degrees” without a unit in that field. The translation does not add one.

The error descriptions indicate SPI communication to hashboards and I2C communication to the power supply. They do not define wire framing, addresses, command bytes, voltages, timing, or the full boot sequence.
