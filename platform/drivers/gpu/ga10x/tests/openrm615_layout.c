/* Contrato de formato contra las cabeceras oficiales OpenRM 615.78.08.
 * Compilar con -fsyntax-only y las rutas indicadas en el informe del 21-10.
 * No copia firmware ni implementacion NVIDIA. Solo comprueba su ABI. */
#include <stddef.h>
#include "nvtypes.h"
#include "nvstatus.h"
#include "gsp/gsp_fw_wpr_meta_v1.h"
#include "rmRiscvUcode.h"
#include "gmcapi/gmcapi_base.h"
#include "rmgspseq.h"
#include "published/ampere/ga102/dev_falcon_v4.h"
#define W(campo, offset) _Static_assert(offsetof(GspFwWprMetaV1, campo) == offset, #campo)
_Static_assert(sizeof(GspFwWprMetaV1) == 256, "WPR V1 size");
W(magic, 0); W(revision, 8);
W(sysmemAddrOfRadix3Elf, 16); W(sizeOfRadix3Elf, 24);
W(sysmemAddrOfBootloader, 32); W(sizeOfBootloader, 40);
W(bootloaderCodeOffset, 48); W(bootloaderDataOffset, 56); W(bootloaderManifestOffset, 64);
W(sysmemAddrOfSignature, 72); W(sizeOfSignature, 80);
W(gspFwRsvdStart, 88); W(nonWprHeapOffset, 96); W(nonWprHeapSize, 104);
W(gspFwWprStart, 112); W(gspFwHeapOffset, 120); W(gspFwHeapSize, 128);
W(gspFwOffset, 136); W(bootBinOffset, 144); W(frtsOffset, 152); W(frtsSize, 160);
W(gspFwWprEnd, 168); W(fbSize, 176); W(vgaWorkspaceOffset, 184); W(vgaWorkspaceSize, 192);
W(bootCount, 200); W(sysmemAddrOfCrashReportQueue, 224); W(sizeOfCrashReportQueue, 232);
W(gspFwHeapVfPartitionCount, 240); W(flags, 241); W(pagingConfig, 242); W(verified, 248);
_Static_assert(offsetof(RM_RISCV_UCODE_DESC, appVersion) == 28, "RISC-V appVersion");
_Static_assert(offsetof(RM_RISCV_UCODE_DESC, manifestOffset) == 32, "RISC-V manifest");
_Static_assert(offsetof(RM_RISCV_UCODE_DESC, monitorDataOffset) == 40, "RISC-V data");
_Static_assert(offsetof(RM_RISCV_UCODE_DESC, monitorCodeOffset) == 48, "RISC-V code");
_Static_assert(sizeof(GMCAPI_HEADER) == 40, "GMCAPI size");
_Static_assert(offsetof(GMCAPI_HEADER, data) == 40, "GMCAPI payload");
_Static_assert(sizeof(GspLoadExecHsBinaryParams) == 80, "HS request size");
_Static_assert(offsetof(GspLoadExecHsBinaryParams, hsSigDmemAddr) == 56, "HS signature");
_Static_assert(NV_PFALCON_FALCON_DMATRFCMD_FULL_TRUE == 1, "FULL true");
_Static_assert(NV_PFALCON_FALCON_DMATRFCMD_IDLE_TRUE == 1, "IDLE true");
