# PSU3300 manual: English reading guide and image-label companion

Original artifact: `mirror-8a07caa0432a1dd0`, `1_Power_supply_simple_maintenance_manual.pdf`, document `QS-AMS-DY-001(A/0)`, 29 pages, dated 2022-03-08. The saved original is a bilingual Canaan document from a public archive. It contains English technical instructions and tables alongside or after the Chinese text.

This companion identifies the English content and translates the Chinese picture labels on page 25. It is not a replacement repair procedure. The original photographs, circuit designators, voltages and document warnings remain in the PDF. AI review; no human review.

## English content already in the original

- Pages 3–8: purpose, supported PSU models, component descriptions, main circuit stages and block diagram. The model list covers PSU3300-03, PSU3300-03 PLUS and PSU3300-01 PLUS1. Input ranges differ.
- Pages 9–22: inspection, fault isolation, circuit checks and model-specific troubleshooting. English instructions follow the Chinese instructions or appear in the same diagram/table.
- Pages 23–24: fault table in Chinese, followed by an English table. The source uses the English heading “Parts to Diagnosed” for components to inspect.
- Pages 25–27: page 25 has a Chinese table embedded as an image. The following pages give the English procedure and corresponding images.
- Pages 28–29: required tools, protection-switch materials and maintenance precautions, with English counterparts.

The extracted text breaks some labels and words across lines because of the layout and watermark. This does not by itself mean that an English counterpart is absent. The correspondence of every small label in every photograph has not been independently checked.

## Page 25 picture labels

| Picture | Chinese label meaning in English |
|---|---|
| Row 1 | The marked area is the U15 PFC circuit. |
| Row 2 | The marked area gives the locations of U12 and U19. |
| Row 3, upper label | Communication daughterboard. |
| Row 3, lower label | Six-pin output cable. |
| Row 4 | The marked areas give the capacitor locations. |

The row headings mean: no output; no main-circuit output; no communication; power-supply failure. The last row heading does not itself define one specific component failure.

## Simulator relevance

The manual distinguishes power conversion, auxiliary power, the MCU control section and protection functions. It also describes indicator blink patterns and a control connector. This evidence can guide a PSU state model. It does not provide a complete command protocol, calibrated power curve, or proof that the listed PSU models can replace each other.
