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
; shader hash: 2f7138d131794e84122d86b50d1114eb
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
;   output 0 depends on inputs: { 4 }
;   output 1 depends on inputs: { 4 }
;   output 2 depends on inputs: { 5 }
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
  %4 = fmul fast float %2, 1.000000e+03
  %5 = fptosi float %4 to i32
  %6 = fmul fast float %3, 1.000000e+05
  %7 = fptoui float %6 to i32
  %8 = call %dx.types.CBufRet.i32 @dx.op.cbufferLoadLegacy.i32(i32 59, %dx.types.Handle %1, i32 1)  ; CBufferLoadLegacy(handle,regIndex)
  %9 = extractvalue %dx.types.CBufRet.i32 %8, 0
  %10 = sdiv i32 %5, %9
  %11 = srem i32 %5, %9
  %12 = extractvalue %dx.types.CBufRet.i32 %8, 1
  %13 = udiv i32 %7, %12
  %14 = urem i32 %7, %12
  %15 = icmp eq i32 %7, 0
  br i1 %15, label %32, label %16

; <label>:16                                      ; preds = %0
  br label %17

; <label>:17                                      ; preds = %17, %16
  %18 = phi i32 [ %26, %17 ], [ 0, %16 ]
  %19 = phi i32 [ %25, %17 ], [ %7, %16 ]
  %20 = phi i32 [ %24, %17 ], [ 0, %16 ]
  %21 = call %dx.types.CBufRet.i32 @dx.op.cbufferLoadLegacy.i32(i32 59, %dx.types.Handle %1, i32 1)  ; CBufferLoadLegacy(handle,regIndex)
  %22 = extractvalue %dx.types.CBufRet.i32 %21, 2
  %23 = urem i32 %19, %22
  %24 = add i32 %23, %20
  %25 = udiv i32 %19, %22
  %26 = add nuw nsw i32 %18, 1
  %27 = icmp slt i32 %26, 8
  %28 = icmp ne i32 %25, 0
  %29 = and i1 %27, %28
  br i1 %29, label %17, label %30, !llvm.loop !18

; <label>:30                                      ; preds = %17
  %31 = uitofp i32 %24 to float
  br label %32

; <label>:32                                      ; preds = %30, %0
  %33 = phi float [ %31, %30 ], [ 0.000000e+00, %0 ]
  %34 = sitofp i32 %10 to float
  %35 = sitofp i32 %11 to float
  %36 = mul i32 %14, 3
  %37 = add i32 %36, %13
  %38 = uitofp i32 %37 to float
  %39 = sdiv i32 %5, 4
  %40 = sitofp i32 %39 to float
  %41 = fadd fast float %40, %33
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 0, float %34)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 1, float %35)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 2, float %38)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  call void @dx.op.storeOutput.f32(i32 5, i32 0, i32 0, i8 3, float %41)  ; StoreOutput(outputSigId,rowIndex,colIndex,value)
  ret void
}

; Function Attrs: nounwind readnone
declare float @dx.op.loadInput.f32(i32, i32, i32, i8, i32) #0

; Function Attrs: nounwind
declare void @dx.op.storeOutput.f32(i32, i32, i32, i8, float) #1

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
!7 = !{[8 x i32] [i32 6, i32 4, i32 0, i32 0, i32 0, i32 0, i32 11, i32 12]}
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
!18 = distinct !{!18, !19}
!19 = !{!"llvm.loop.unroll.disable"}

