# Windows LibreOffice source patch 0053

`0053-sal-long-path-threshold-boundary.patch` is copied byte-for-byte from the provenance attachment in the 2026-10-11 SAL boundary stage. Its SHA-256 is `66FA79185CB10E911FB23934CEB4BC76505BB7E0F74F7AEE1E0B010C146D8DD7`.

The reviewed base is LibreOffice core revision `8221e31b3ac356a1623c672912a3d2b492f7e3d1`. The patch changes two comparisons in `sal/osl/w32/file_url.cxx`, from `<= MAX_PATH - 12` to `< MAX_PATH - 12`. The source file SHA-256 recorded in the portable provenance is `3A61CF7B0DD0D68AB5CB3D31C31371493F5E88CA8749F301BC57AFCF858705A8`. Upstream source: [LibreOffice core at the reviewed revision](https://github.com/LibreOffice/core/blob/8221e31b3ac356a1623c672912a3d2b492f7e3d1/sal/osl/w32/file_url.cxx).

The prior native validation report records that, against this revision, the SAL boundary test changed from a 248-character plain path failing to succeeding with the extended prefix. In the compared 4,209-file runtime manifest, only `program/sal3.dll` changed (`B0C2E70AB2EB5AA1DAD5D5CA0DBFEE6AEF259A2DEE3030490A1290DB44EFC1F6`); `program/mergedlo.dll` remained `FE1109F8C6831CF1C10D29D4F65683A54BC022D7A587520D504D5BBB46A90F70`, and 597 named exports/ordinals had no differences. Those are prior stage results; this source candidate did not repeat the native build or test.

This file is kept separate from the baseline `patches/libreoffice/0001`–`0039` chain. Apply it only to the reviewed Windows LibreOffice source revision after checking the source tree and its other patches. The supplied SAL provenance does not include patches `0040`–`0052` or a complete native LibreOffice build recipe. LibreOffice's own license and notices continue to apply; see [`docs/licenses.md`](../../../docs/licenses.md).
