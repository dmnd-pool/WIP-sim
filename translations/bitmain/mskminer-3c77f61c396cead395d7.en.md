# Antminer 9-series installation with an SD card

English translation of the Russian MSKMINER document. Scope: all extractable body text on pages 1–4. The two JP4 position photographs remain in the original PDF. This translation does not infer a jumper pin number from a photograph. Translation date: 2026-10-19. These are historical vendor instructions, not a tested procedure for an arbitrary control board.

1. Download the firmware for your ASIC from https://mskminer.com/ru/. Select the SD-card version. It is a ZIP archive. The source calls the web-update extension `tar.zg`; this appears to be a spelling error for `tar.gz`.
2. Format an SD card with FAT32. Its capacity must not exceed 16 GB. Extract the archive to the card.
3. Set control-board jumper JP4 to the SD-card installation position. Page 2 shows the normal position. Page 3 shows the SD-card installation position. Use the original photographs and the exact control-board instructions.
4. Insert the card into the control-board SD slot. Apply power. The red and green LEDs flash together when installation starts. They flash in a cycle when installation is complete.
5. Remove power. Return JP4 to its original position. Remove the SD card. The source then states that the ASIC is ready to operate.
