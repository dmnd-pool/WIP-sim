# Antminer 17-series installation with an SD card

English translation of the Russian MSKMINER document. Scope: all extractable body text on pages 1–2. Screenshots remain in the original PDF. Translation date: 2026-10-19. These are historical vendor instructions, not a tested procedure for an arbitrary control board.

1. Download the firmware for your ASIC from https://mskminer.com/ru/. Select the SD-card version. It is a ZIP archive. The source calls the web-update extension `tar.zg`; this appears to be a spelling error for `tar.gz`.
2. Format an SD card with FAT32. Its capacity must not exceed 16 GB. Extract the archive to the card.
3. Insert the card into the SD slot on the front panel. Apply power. The red and green LEDs flash together when installation starts. They flash in a cycle when installation is complete.
4. After installation is complete, remove power and remove the SD card. The source then states that the ASIC is ready to operate.
