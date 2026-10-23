# SF3301 verification-board hardware guide: English companion

Source: `SF3301_SV_Board_Hardware_Guide.doc` inside artifact `vendors-sfards-sf3301-source`. The source has parallel English and Chinese body text. This companion covers the complete body requirements and all Chinese annotations in the board photograph. The photograph was visually checked. PCB component markings remain unchanged. AI translation; not human reviewed.

Minimum requirements:

1. An x86 Linux system with at least 4 GB of memory.
2. Two USB-to-UART serial cables for the RS232 connections.
3. A 12 V AC-to-DC power supply rated at least 120 W.
4. One 2.5 mm power connector.

The [original board photograph](../../work/vendors/legacy-docs/SF3301-hardware-guide-image.png) has these annotations:

| Position or English label | Full English meaning of Chinese annotation |
|---|---|
| LTC power board | Litecoin power board. |
| V-L Configuration | Litecoin voltage fine adjustment. |
| Left lower 12 V input | Supply the Litecoin power board with 12 V. Observe positive and negative polarity. |
| LTC test point | Litecoin voltage measurement. |
| FBB jumper | Bias-voltage jumper. |
| SV power 12 V | I/O supply: 12 V, center positive and outer contact negative. |
| BTC power board | Bitcoin power board. |
| Right upper 12 V input | Bitcoin power-board supply. Observe positive and negative polarity. |
| BTC test point | Bitcoin voltage measurement. |
| V-b configuration | Bitcoin voltage fine adjustment. |
| LTC UART | Litecoin serial port. |
| BTC UART | Bitcoin serial port. |

This is a translation of the original reference-board guide. It does not establish the electrical limits of an SF100 production board or any later hardware revision.
