# Windows LibreOffice 源码补丁 0053 | Windows LibreOffice source patch 0053

`0053-sal-long-path-threshold-boundary.patch` 从 2026-10-11 SAL 边界测试 stage 的 provenance 附件逐字复制。SHA-256：`66FA79185CB10E911FB23934CEB4BC76505BB7E0F74F7AEE1E0B010C146D8DD7`。

`0053-sal-long-path-threshold-boundary.patch` was copied byte-for-byte from the provenance attachment in the 2026-10-11 SAL boundary-test stage. SHA-256: `66FA79185CB10E911FB23934CEB4BC76505BB7E0F74F7AEE1E0B010C146D8DD7`.

审阅基线为 LibreOffice core revision `8221e31b3ac356a1623c672912a3d2b492f7e3d1`。补丁在 `sal/osl/w32/file_url.cxx` 中将两处比较从 `<= MAX_PATH - 12` 改为 `< MAX_PATH - 12`。来源文件 SHA-256 记录为 `3A61CF7B0DD0D68AB5CB3D31C31371493F5E88CA8749F301BC57AFCF858705A8`。上游文件：[LibreOffice core at the reviewed revision](https://github.com/LibreOffice/core/blob/8221e31b3ac356a1623c672912a3d2b492f7e3d1/sal/osl/w32/file_url.cxx)。

The reviewed base is LibreOffice core revision `8221e31b3ac356a1623c672912a3d2b492f7e3d1`. In `sal/osl/w32/file_url.cxx`, the patch changes two comparisons from `<= MAX_PATH - 12` to `< MAX_PATH - 12`. The source file SHA-256 recorded in provenance is `3A61CF7B0DD0D68AB5CB3D31C31371493F5E88CA8749F301BC57AFCF858705A8`. Upstream source: [LibreOffice core at the reviewed revision](https://github.com/LibreOffice/core/blob/8221e31b3ac356a1623c672912a3d2b492f7e3d1/sal/osl/w32/file_url.cxx).

此前 native validation 报告记载：在该 revision 上，SAL 边界测试从 248 字符普通路径失败变为成功使用 extended prefix；对比的 4,209 文件 runtime manifest 中，只有 `program/sal3.dll` 改变（`B0C2E70AB2EB5AA1DAD5D5CA0DBFEE6AEF259A2DEE3030490A1290DB44EFC1F6`）；`program/mergedlo.dll` 保持 `FE1109F8C6831CF1C10D29D4F65683A54BC022D7A587520D504D5BBB46A90F70`，597 个命名导出/序号无差异。这些是此前 stage 的结果；本轮整合没有重新执行原生构建或 SAL 测试。

The earlier native-validation report records that, at this revision, the SAL boundary test changed from failing on a 248-character plain path to succeeding with the extended prefix. In the compared 4,209-file runtime manifest, only `program/sal3.dll` changed (`B0C2E70AB2EB5AA1DAD5D5CA0DBFEE6AEF259A2DEE3030490A1290DB44EFC1F6`); `program/mergedlo.dll` remained `FE1109F8C6831CF1C10D29D4F65683A54BC022D7A587520D504D5BBB46A90F70`, and there were no differences in 597 named exports/ordinals. These are earlier stage results; this integration did not repeat the native build or SAL test.

此补丁与默认跨平台 `patches/libreoffice/0001`–`0039` 补丁链分开保存，只适用于上述审阅过的 Windows LibreOffice revision。现有来源材料没有 `0040`–`0052` 补丁原件，也没有完整原生 Windows LibreOffice 构建流程，因此本仓库不能据此完整复现 native Windows LibreOffice runtime。LibreOffice 自身的许可和声明仍适用；参见 [docs/licenses.md](../../../docs/licenses.md)。

This patch is kept separate from the default cross-platform `patches/libreoffice/0001`–`0039` chain and applies only to the reviewed Windows LibreOffice revision above. The available provenance does not include original patches `0040`–`0052` or a complete native Windows LibreOffice build procedure, so the native Windows LibreOffice runtime cannot be fully reproduced from the material here. LibreOffice's own license and notices continue to apply; see [docs/licenses.md](../../../docs/licenses.md).
