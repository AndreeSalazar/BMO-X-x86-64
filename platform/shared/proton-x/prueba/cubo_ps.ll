;
; Input signature:
;
; Name                 Index   Mask Register SysValue  Format   Used
; -------------------- ----- ------ -------- -------- ------- ------
; SV_Position              0   xyzw        0      POS   float       
; NORMAL                   0   xyz         1     NONE   float   xyz 
; COLOR                    0   xyzw        2     NONE   float   xyzw
;
;
; Output signature:
;
; Name                 Index   Mask Register SysValue  Format   Used
; -------------------- ----- ------ -------- -------- ------- ------
; SV_Target                0   xyzw        0   TARGET   float   xyzw
;
; shader hash: 6a60c2379d1377d6c752991135b3a96f
;
; Pipeline Runtime Information: 
;
; Pixel Shader
; DepthOutput=0
; SampleFrequency=0
;
;
; Input signature:
;
; Name                 Index             InterpMode DynIdx
; -------------------- ----- ---------------------- ------
; SV_Position              0          noperspective       
; NORMAL                   0                 linear       
; COLOR                    0                 linear       
;
; Output signature:
;
; Name                 Index             InterpMode DynIdx
; -------------------- ----- ---------------------- ------
; SV_Target                0                              
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
; Number of inputs: 12, outputs: 4
; Outputs dependent on ViewId: {  }
; Inputs contributing to computation of Outputs:
;   output 0 depends on inputs: { 4, 5, 6, 8 }
;   output 1 depends on inputs: { 4, 5, 6, 9 }
;   output 2 depends on inputs: { 4, 5, 6, 10 }
;   output 3 depends on inputs: { 11 }
;
target datalayout = "e-m:e-p:32:32-i1:32-i8:32-i16:32-i32:32-i64:64-f16:32-f32:32-f64:64-n8:16:32:64"
target triple = "dxil-ms-dx"

%dx.types.Handle = type { i8* }
%dx.types.CBufRet.f32 = type { float, float, float, float }
%hostlayout.Constantes = type { [4 x <4 x float>], [4 x <4 x float>], <4 x float> }

define void @pixel() {
  %1 = call %dx.types.Handle @dx.op.createHandle(i32 57, i8 2, i32 0, i32 0, i1 false)  ; CreateHandle(resourceClass,rangeId,index,nonUniformIndex)
  %2 = call float @dx.op.loadInput.f32(i32 4, i32 2, i32 0, i8 0, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %3 = call float @dx.op.loadInput.f32(i32 4, i32 2, i32 0, i8 1, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %4 = call float @dx.op.loadInput.f32(i32 4, i32 2, i32 0, i8 2, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %5 = call float @dx.op.loadInput.f32(i32 4, i32 2, i32 0, i8 3, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %6 = call float @dx.op.loadInput.f32(i32 4, i32 1, i32 0, i8 0, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %7 = call float @dx.op.loadInput.f32(i32 4, i32 1, i32 0, i8 1, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %8 = call float @dx.op.loadInput.f32(i32 4, i32 1, i32 0, i8 2, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %9 = call %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32 59, %dx.types.Handle %1, i32 8)  ; CBufferLoadLegacy(handle,regIndex)
  %10 = extractvalue %dx.types.CBufRet.f32 %9, 0
  %11 = extractvalue %dx.types.CBufRet.f32 %9, 1
  %12 = extractvalue %dx.types.CBufRet.f32 %9, 2
  %13 = call float @dx.op.dot3.f32(i32 55, float %6, float %7, float %8, float %6, float %7, float %8)  ; Dot3(ax,ay,az,bx,by,bz)
  %14 = call float @dx.op.unary.f32(i32 25, float %13)  ; Rsqrt(value)
  %15 = fmul fast float %14, %6
  %16 = fmul fast float %14, %7
  %17 = fmul fast float %14, %8
  %18 = call float @dx.op.dot3.f32(i32 55, float %15, float %16, float %17, float %10, float %11, float %12)  ; Dot3(ax,ay,az,bx,by,bz)
  %19 = call float @dx.op.unary.f32(i32 7, float %18)  ; Saturate(value)
  %20 = extractvalue %dx.types.CBufRet.f32 %9, 3
  %21 = fsub fast float 1.000000e+00, %20
  %22 = fmul fast float %19, %21
  %23 = fadd fast float %22, %20
  %24 = fmul fast float %23, %2
  %25 = fmul fast float %23, %3
  %26 = fmul fast float %23, %4
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 0, float %24)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 1, float %25)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 2, float %26)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 3, float %5)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  ret void
}

; Function Attrs: nounwind readnone
declare float @dx.op.loadInput.f32(i32, i32, i32, i8, i32) #0

; Function Attrs: nounwind
declare void @dx.op.storeOutput.f32(i32, i32, i32, i8, float) #1

; Function Attrs: nounwind readnone
declare float @dx.op.unary.f32(i32, float) #0

; Function Attrs: nounwind readnone
declare float @dx.op.dot3.f32(i32, float, float, float, float, float, float) #0

; Function Attrs: nounwind readonly
declare %dx.types.CBufRet.f32 @dx.op.cbufferLoadLegacy.f32(i32, %dx.types.Handle, i32) #2

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
!3 = !{!"ps", i32 6, i32 0}
!4 = !{null, null, !5, null}
!5 = !{!6}
!6 = !{i32 0, %hostlayout.Constantes* undef, !"", i32 0, i32 0, i32 1, i32 144, null}
!7 = !{[14 x i32] [i32 12, i32 4, i32 0, i32 0, i32 0, i32 0, i32 7, i32 7, i32 7, i32 0, i32 1, i32 2, i32 4, i32 8]}
!8 = !{void ()* @pixel, !"pixel", !9, !4, null}
!9 = !{!10, !17, null}
!10 = !{!11, !13, !15}
!11 = !{i32 0, !"SV_Position", i8 9, i8 3, !12, i8 4, i32 1, i8 4, i32 0, i8 0, null}
!12 = !{i32 0}
!13 = !{i32 1, !"NORMAL", i8 9, i8 0, !12, i8 2, i32 1, i8 3, i32 1, i8 0, !14}
!14 = !{i32 3, i32 7}
!15 = !{i32 2, !"COLOR", i8 9, i8 0, !12, i8 2, i32 1, i8 4, i32 2, i8 0, !16}
!16 = !{i32 3, i32 15}
!17 = !{!18}
!18 = !{i32 0, !"SV_Target", i8 9, i8 16, !12, i8 0, i32 1, i8 4, i32 0, i8 0, !16}

