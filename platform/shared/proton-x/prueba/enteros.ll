;
; Input signature:
;
; Name                 Index   Mask Register SysValue  Format   Used
; -------------------- ----- ------ -------- -------- ------- ------
; SV_Position              0   xyzw        0      POS   float       
; TEXCOORD                 0   xy          1     NONE   float   xy  
;
;
; Output signature:
;
; Name                 Index   Mask Register SysValue  Format   Used
; -------------------- ----- ------ -------- -------- ------- ------
; SV_Target                0   xyzw        0   TARGET   float   xyzw
;
; shader hash: 02063c582aff6785f03160a997c2dd7c
;
; Pipeline Runtime Information: 
;
;PSVRuntimeInfo:
; Pixel Shader
; DepthOutput=0
; SampleFrequency=0
; MinimumExpectedWaveLaneCount: 0
; MaximumExpectedWaveLaneCount: 4294967295
; UsesViewID: false
; SigInputElements: 2
; SigOutputElements: 1
; SigPatchConstOrPrimElements: 0
; SigInputVectors: 2
; SigOutputVectors[0]: 1
; SigOutputVectors[1]: 0
; SigOutputVectors[2]: 0
; SigOutputVectors[3]: 0
; EntryFunctionName: pixel
;
;
; Input signature:
;
; Name                 Index             InterpMode DynIdx
; -------------------- ----- ---------------------- ------
; SV_Position              0          noperspective       
; TEXCOORD                 0                 linear       
;
; Output signature:
;
; Name                 Index             InterpMode DynIdx
; -------------------- ----- ---------------------- ------
; SV_Target                0                              
;
; Buffer Definitions:
;
; cbuffer B
; {
;
;   struct B
;   {
;
;       float4 k;                                     ; Offset:    0
;       int4 n;                                       ; Offset:   16
;   
;   } B;                                              ; Offset:    0 Size:    32
;
; }
;
;
; Resource Bindings:
;
; Name                                 Type  Format         Dim      ID      HLSL Bind  Count
; ------------------------------ ---------- ------- ----------- ------- -------------- ------
; B                                 cbuffer      NA          NA     CB0            cb0     1
;
;
; ViewId state:
;
; Number of inputs: 6, outputs: 4
; Outputs dependent on ViewId: {  }
; Inputs contributing to computation of Outputs:
;   output 0 depends on inputs: { 4, 5 }
;   output 1 depends on inputs: { 4, 5 }
;   output 2 depends on inputs: { 4, 5 }
;   output 3 depends on inputs: { 4, 5 }
;
target datalayout = "e-m:e-p:32:32-i1:32-i8:32-i16:32-i32:32-i64:64-f16:32-f32:32-f64:64-n8:16:32:64"
target triple = "dxil-ms-dx"

%dx.types.Handle = type { i8* }
%dx.types.CBufRet.i32 = type { i32, i32, i32, i32 }
%B = type { <4 x float>, <4 x i32> }

define void @pixel() {
  %1 = call %dx.types.Handle @dx.op.createHandle(i32 57, i8 2, i32 0, i32 0, i1 false)  ; CreateHandle(resourceClass,rangeId,index,nonUniformIndex)
  %2 = call float @dx.op.loadInput.f32(i32 4, i32 1, i32 0, i8 0, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %3 = call float @dx.op.loadInput.f32(i32 4, i32 1, i32 0, i8 1, i32 undef)  ; LoadInput(inputSigId,rowIndex,colIndex,gsVertexAxis)
  %4 = fmul fast float %2, 1.600000e+01
  %5 = fptosi float %4 to i32
  %6 = fmul fast float %3, 8.000000e+00
  %7 = fptoui float %6 to i32
  %8 = call %dx.types.CBufRet.i32 @dx.op.cbufferLoadLegacy.i32(i32 59, %dx.types.Handle %1, i32 1)  ; CBufferLoadLegacy(handle,regIndex)
  %9 = extractvalue %dx.types.CBufRet.i32 %8, 0
  %10 = add i32 %9, 4
  %11 = mul i32 %10, %5
  %12 = lshr i32 %7, 1
  %13 = sub nsw i32 %11, %12
  %14 = xor i32 %7, 5
  %15 = and i32 %5, 15
  %16 = or i32 %14, %15
  %17 = extractvalue %dx.types.CBufRet.i32 %8, 2
  %18 = extractvalue %dx.types.CBufRet.i32 %8, 1
  %19 = call i32 @dx.op.binary.i32(i32 37, i32 %13, i32 %18)  ; IMax(a,b)
  %20 = call i32 @dx.op.binary.i32(i32 38, i32 %19, i32 %17)  ; IMin(a,b)
  %21 = extractvalue %dx.types.CBufRet.i32 %8, 3
  %22 = call i32 @dx.op.binary.i32(i32 40, i32 %16, i32 %21)  ; UMin(a,b)
  %23 = and i32 %5, 3
  switch i32 %23, label %32 [
    i32 0, label %24
    i32 1, label %26
    i32 2, label %29
    i32 3, label %29
  ], !dx.controlflow.hints !18

; <label>:24                                      ; preds = %0
  %25 = sitofp i32 %13 to float
  br label %32

; <label>:26                                      ; preds = %0
  %27 = uitofp i32 %16 to float
  %28 = fmul fast float %27, 5.000000e-01
  br label %32

; <label>:29                                      ; preds = %0, %0
  %30 = sub nsw i32 %20, %22
  %31 = sitofp i32 %30 to float
  br label %32

; <label>:32                                      ; preds = %29, %26, %24, %0
  %33 = phi float [ %31, %29 ], [ %28, %26 ], [ %25, %24 ], [ -1.000000e+00, %0 ]
  %34 = and i32 %7, 1
  %35 = icmp eq i32 %34, 0
  %36 = sitofp i32 %20 to float
  %37 = uitofp i32 %22 to float
  %38 = ashr i32 %13, 3
  %39 = sitofp i32 %38 to float
  %40 = select i1 %35, float 1.000000e+00, float %39
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 0, float %33)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 1, float %36)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 2, float %37)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 3, float %40)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  ret void
}

; Function Attrs: nounwind readnone
declare float @dx.op.loadInput.f32(i32, i32, i32, i8, i32) #0

; Function Attrs: nounwind
declare void @dx.op.storeOutput.f32(i32, i32, i32, i8, float) #1

; Function Attrs: nounwind readnone
declare i32 @dx.op.binary.i32(i32, i32, i32) #0

; Function Attrs: nounwind readonly
declare %dx.types.CBufRet.i32 @dx.op.cbufferLoadLegacy.i32(i32, %dx.types.Handle, i32) #2

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

!0 = !{!"dxc(private) 1.8.0.4946 (9efbb6c32)"}
!1 = !{i32 1, i32 0}
!2 = !{i32 1, i32 9}
!3 = !{!"ps", i32 6, i32 0}
!4 = !{null, null, !5, null}
!5 = !{!6}
!6 = !{i32 0, %B* undef, !"", i32 0, i32 0, i32 1, i32 32, null}
!7 = !{[8 x i32] [i32 6, i32 4, i32 0, i32 0, i32 0, i32 0, i32 15, i32 15]}
!8 = !{void ()* @pixel, !"pixel", !9, !4, null}
!9 = !{!10, !15, null}
!10 = !{!11, !13}
!11 = !{i32 0, !"SV_Position", i8 9, i8 3, !12, i8 4, i32 1, i8 4, i32 0, i8 0, null}
!12 = !{i32 0}
!13 = !{i32 1, !"TEXCOORD", i8 9, i8 0, !12, i8 2, i32 1, i8 2, i32 1, i8 0, !14}
!14 = !{i32 3, i32 3}
!15 = !{!16}
!16 = !{i32 0, !"SV_Target", i8 9, i8 16, !12, i8 0, i32 1, i8 4, i32 0, i8 0, !17}
!17 = !{i32 3, i32 15}
!18 = distinct !{!18, !"dx.controlflow.hints", i32 1}
