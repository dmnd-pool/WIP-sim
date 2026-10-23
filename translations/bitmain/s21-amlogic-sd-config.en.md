# S21 Amlogic SD image configuration — English companion

Source artifact: `quanthash-5c9baddc5fcc778707c3`.
Archive member: `aml_sdc_burn.ini` in `S21-aml-flashcard.zip`.
Source encoding: GB18030-compatible Chinese comments with ASCII configuration.
Scope: complete translation of all comments; all setting names and values are preserved.
The original archive and extracted member remain unchanged. This file is documentation. It is not an installation instruction.

The header identifies an Amlogic SD-card image programming configuration script.
It accepts DOS and Unix line formats. The Chinese comment recommends that the file remain in DOS format.
Do not edit a file in Unix format with Windows Notepad, because the edited file can become unreadable by the parser.
All characters outside comment lines must be readable ASCII characters.

```ini
[common]
erase_bootloader    =1
erase_flash         =1
reboot              =1

; The SD-card programming tool fills the package value.
[burn_ex]
package     =aml_upgrade_package_enc.img
;media       =
```

The values request bootloader erase, flash erase, and restart. The package name identifies an encrypted image. The catalogue did not execute this configuration or decrypt the image.
