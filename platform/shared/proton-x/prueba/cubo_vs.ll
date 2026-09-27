;
; Input signature:
;
; Name                 Index   Mask Register SysValue  Format   Used
; -------------------- ----- ------ -------- -------- ------- ------
; POSITION                 0   xyz         0     NONE   float   xyz 
; NORMAL                   0   xyz         1     NONE   float   xyz 
; COLOR                    0   xyzw        2     NONE   float   xyzw
;
;
; Output signature:
;
; Name                 Index   Mask Register SysValue  Format   Used
; -------------------- ----- ------ -------- -------- ------- ------
; SV_Position              0   xyzw        0      POS   float   xyzw
; NORMAL                   0   xyz         1     NONE   float   xyz 
; COLOR                    0   xyzw        2     NONE   float   xyzw
;
; shader hash: d45285fbeb91a618638d0a5a2da83eca
;
; Pipeline Runtime Information: 
;
; Vertex Shader
; OutputPositionPresent=1
;
;
; Input signature:
;
; Name                 Index             InterpMode DynIdx
; -------------------- ----- ---------------------- ------
; POSITION                 0                              
; NORMAL                   0                              
; COLOR                    0                              
;
; Output signature:
;
; Name                 Index             InterpMode DynIdx
; -------------------- ----- ---------------------- ------
; SV_Position              0          noperspective       
; NORMAL                   0                 linear       
; COLOR                    0                 linear       
;
; Buffer Definitions:
;
; cbuffer Constantes
; {
;
;   struct hostlayout.Constantes
;   {
;
;       column_major float4x4 wvp;                    ; Offset:    0
;       column_major float4x4 world;                  ; Offset:   64
;       float4 luz;                                   ; Offset:  128
;   
;   } Constantes;                                     ; Offset:    0 Size:   144
;
; }
;
;
; Resource Bindings:
;
; Name                                 Type  Format         Dim      ID      HLSL Bind  Count
; ------------------------------ ---------- ------- ----------- ------- -------------- ------
; Constantes                        cbuffer      NA          NA     CB0            cb0     1
;
;
; ViewId state:
;
; Number of inputs: 12, outputs: 12
; Outputs dependent on ViewId: {  }
; Inputs contributing to computation of Outputs:
;   output 0 depends on inputs: { 0, 1, 2 }
;   output 1 depends on inputs: { 0, 1, 2 }
;   output 2 depends on inputs: { 0, 1, 2 }
;   output 3 depends on inputs: { 0, 1, 2 }
;   output 4 depends on inputs: { 4, 5, 6 }
;   output 5 depends on inputs: { 4, 5, 6 }
;   output 6 depends on inputs: { 4, 5, 6 }
;   output 8 depends on inputs: { 8 }
;   output 9 depends on inputs: { 9 }
;   output 10 depends on inputs: { 10 }
;   output 11 depends on inputs: { 11 }
;
target datalayout = "e-m:e-p:32:32-i1:32-i8:32-i16:32-i32:32-i64:64-f16:32-f32:32-f64:64-n8:16:32:64"
target triple = "dxil-ms-dx"

%dx.types.Handle = type { i8* }
%dx.types.CBufRet.f32 = type { float, float, float, float }
%hostlayout.Constantes = type { [4 x <4 x float>], [4 x <4 x float>], <4 x float> }

define void @vertice() {
  %1 = call %dx.types.Handle @dx.op.createHandle(i32 57, i8 2, i32 0, i32 0, i1 false)  ; CreateHandle(resourceClass,rangeId,index,nonUniformIndex)
  %2 = call float @dx.op.loadInput.f32(i32 4, i32 2, i32 0, i8 0, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %3 = call float @dx.op.loadInput.f32(i32 4, i32 2, i32 0, i8 1, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %4 = call float @dx.op.loadInput.f32(i32 4, i32 2, i32 0, i8 2, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %5 = call float @dx.op.loadInput.f32(i32 4, i32 2, i32 0, i8 3, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %6 = call float @dx.op.loadInput.f32(i32 4, i32 1, i32 0, i8 0, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %7 = call float @dx.op.loadInput.f32(i32 4, i32 1, i32 0, i8 1, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %8 = call float @dx.op.loadInput.f32(i32 4, i32 1, i32 0, i8 2, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %9 = call float @dx.op.loadInput.f32(i32 4, i32 0, i32 0, i8 0, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %10 = call float @dx.op.loadInput.f32(i32 4, i32 0, i32 0, i8 1, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %11 = call float @dx.op.loadInput.f32(i32 4, i32 0, i32 0, i8 2, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %12 = call %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32 59, %dx.types.Handle %1, i32 0)  ; CBufferLoadLegacy(handle,regIndex)
  %13 = extractvalue %dx.types.CBufRet.f32 %12, 0
  %14 = extractvalue %dx.types.CBufRet.f32 %12, 1
  %15 = extractvalue %dx.types.CBufRet.f32 %12, 2
  %16 = extractvalue %dx.types.CBufRet.f32 %12, 3
  %17 = call %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32 59, %dx.types.Handle %1, i32 1)  ; CBufferLoadLegacy(handle,regIndex)
  %18 = extractvalue %dx.types.CBufRet.f32 %17, 0
  %19 = extractvalue %dx.types.CBufRet.f32 %17, 1
  %20 = extractvalue %dx.types.CBufRet.f32 %17, 2
  %21 = extractvalue %dx.types.CBufRet.f32 %17, 3
  %22 = call %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32 59, %dx.types.Handle %1, i32 2)  ; CBufferLoadLegacy(handle,regIndex)
  %23 = extractvalue %dx.types.CBufRet.f32 %22, 0
  %24 = extractvalue %dx.types.CBufRet.f32 %22, 1
  %25 = extractvalue %dx.types.CBufRet.f32 %22, 2
  %26 = extractvalue %dx.types.CBufRet.f32 %22, 3
  %27 = call %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32 59, %dx.types.Handle %1, i32 3)  ; CBufferLoadLegacy(handle,regIndex)
  %28 = extractvalue %dx.types.CBufRet.f32 %27, 0
  %29 = extractvalue %dx.types.CBufRet.f32 %27, 1
  %30 = extractvalue %dx.types.CBufRet.f32 %27, 2
  %31 = extractvalue %dx.types.CBufRet.f32 %27, 3
  %32 = fmul fast float %13, %9
  %33 = call float @dx.op.tertiary.f32(i32 46, float %18, float %10, float %32)  ; FMad(a,b,c)
  %34 = call float @dx.op.tertiary.f32(i32 46, float %23, float %11, float %33)  ; FMad(a,b,c)
  %35 = fadd fast float %34, %28
  %36 = fmul fast float %14, %9
  %37 = call float @dx.op.tertiary.f32(i32 46, float %19, float %10, float %36)  ; FMad(a,b,c)
  %38 = call float @dx.op.tertiary.f32(i32 46, float %24, float %11, float %37)  ; FMad(a,b,c)
  %39 = fadd fast float %38, %29
  %40 = fmul fast float %15, %9
  %41 = call float @dx.op.tertiary.f32(i32 46, float %20, float %10, float %40)  ; FMad(a,b,c)
  %42 = call float @dx.op.tertiary.f32(i32 46, float %25, float %11, float %41)  ; FMad(a,b,c)
  %43 = fadd fast float %42, %30
  %44 = fmul fast float %16, %9
  %45 = call float @dx.op.tertiary.f32(i32 46, float %21, float %10, float %44)  ; FMad(a,b,c)
  %46 = call float @dx.op.tertiary.f32(i32 46, float %26, float %11, float %45)  ; FMad(a,b,c)
  %47 = fadd fast float %46, %31
  %48 = call %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32 59, %dx.types.Handle %1, i32 4)  ; CBufferLoadLegacy(handle,regIndex)
  %49 = extractvalue %dx.types.CBufRet.f32 %48, 0
  %50 = extractvalue %dx.types.CBufRet.f32 %48, 1
  %51 = extractvalue %dx.types.CBufRet.f32 %48, 2
  %52 = call %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32 59, %dx.types.Handle %1, i32 5)  ; CBufferLoadLegacy(handle,regIndex)
  %53 = extractvalue %dx.types.CBufRet.f32 %52, 0
  %54 = extractvalue %dx.types.CBufRet.f32 %52, 1
  %55 = extractvalue %dx.types.CBufRet.f32 %52, 2
  %56 = call %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32 59, %dx.types.Handle %1, i32 6)  ; CBufferLoadLegacy(handle,regIndex)
  %57 = extractvalue %dx.types.CBufRet.f32 %56, 0
  %58 = extractvalue %dx.types.CBufRet.f32 %56, 1
  %59 = extractvalue %dx.types.CBufRet.f32 %56, 2
  %60 = fmul fast float %49, %6
  %61 = call float @dx.op.tertiary.f32(i32 46, float %53, float %7, float %60)  ; FMad(a,b,c)
  %62 = call float @dx.op.tertiary.f32(i32 46, float %57, float %8, float %61)  ; FMad(a,b,c)
  %63 = fmul fast float %50, %6
  %64 = call float @dx.op.tertiary.f32(i32 46, float %54, float %7, float %63)  ; FMad(a,b,c)
  %65 = call float @dx.op.tertiary.f32(i32 46, float %58, float %8, float %64)  ; FMad(a,b,c)
  %66 = fmul fast float %51, %6
  %67 = call float @dx.op.tertiary.f32(i32 46, float %55, float %7, float %66)  ; FMad(a,b,c)
  %68 = call float @dx.op.tertiary.f32(i32 46, float %59, float %8, float %67)  ; FMad(a,b,c)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 0, float %35)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 1, float %39)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 2, float %43)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 3, float %47)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 1, i32 0, i8 0, float %62)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 1, i32 0, i8 1, float %65)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 1, i32 0, i8 2, float %68)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 2, i32 0, i8 0, float %2)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 2, i32 0, i8 1, float %3)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 2, i32 0, i8 2, float %4)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 2, i32 0, i8 3, float %5)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  ret void
}

; Function Attrs: nounwind readnone
declare float @dx.op.loadInput.f32(i32, i32, i32, i8, i32) #0

; Function Attrs: nounwind
declare void @dx.op.storeOutput.f32(i32, i32, i32, i8, float) #1

; Function Attrs: nounwind readonly
declare %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32, %dx.types.Handle, i32) #2

; Function Attrs: nounwind readnone
declare float @dx.op.tertiary.f32(i32, float, float, float) #0

; Function Attrs: nounwind readonly
declare %dx.types.Handle @dx.op.createHandle(i32, i8, i32, i32, i1) #2

attributes #0 = { nounwind readnone }
attributes #1 = { nounwind }
attributes #2 = { nounwind readonly }

!llvm.ident = !{!0}
!dx.version = !{!1}
!dx.valver = !{!2}
!dx.shaderModel = !{!3}
!dx.resources = !{!4}
!dx.viewIdState = !{!7}
!dx.entryPoints = !{!8}

!0 = !{!"dxc(private) 1.8.0.4662 (416fab6b5)"}
!1 = !{i32 1, i32 0}
!2 = !{i32 1, i32 8}
!3 = !{!"vs", i32 6, i32 0}
!4 = !{null, null, !5, null}
!5 = !{!6}
!6 = !{i32 0, %hostlayout.Constantes* undef, !"", i32 0, i32 0, i32 1, i32 144, null}
!7 = !{[14 x i32] [i32 12, i32 12, i32 15, i32 15, i32 15, i32 0, i32 112, i32 112, i32 112, i32 0, i32 256, i32 512, i32 1024, i32 2048]}
!8 = !{void ()* @vertice, !"vertice", !9, !4, null}
!9 = !{!10, !17, null}
!10 = !{!11, !14, !15}
!11 = !{i32 0, !"POSITION", i8 9, i8 0, !12, i8 0, i32 1, i8 3, i32 0, i8 0, !13}
!12 = !{i32 0}
!13 = !{i32 3, i32 7}
!14 = !{i32 1, !"NORMAL", i8 9, i8 0, !12, i8 0, i32 1, i8 3, i32 1, i8 0, !13}
!15 = !{i32 2, !"COLOR", i8 9, i8 0, !12, i8 0, i32 1, i8 4, i32 2, i8 0, !16}
!16 = !{i32 3, i32 15}
!17 = !{!18, !19, !20}
!18 = !{i32 0, !"SV_Position", i8 9, i8 3, !12, i8 4, i32 1, i8 4, i32 0, i8 0, !16}
!19 = !{i32 1, !"NORMAL", i8 9, i8 0, !12, i8 2, i32 1, i8 3, i32 1, i8 0, !13}
!20 = !{i32 2, !"COLOR", i8 9, i8 0, !12, i8 2, i32 1, i8 4, i32 2, i8 0, !16}

