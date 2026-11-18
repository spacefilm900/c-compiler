source_filename = "01_basic.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

declare i32 @putchar(i32)

define internal i64 @scale(i16 signext %p0, i8 zeroext %p1) {
entry:
  %v1 = alloca i16, align 2
  %v2 = alloca i8, align 1
  %v3 = alloca i64, align 8
  store i16 %p0, ptr %v1, align 2
  store i8 %p1, ptr %v2, align 1
  %t1 = load i16, ptr %v1, align 2
  %t2 = sext i16 %t1 to i32
  %t3 = load i8, ptr %v2, align 1
  %t4 = zext i8 %t3 to i32
  %t5 = mul i32 %t2, %t4
  %t6 = sext i32 %t5 to i64
  store i64 %t6, ptr %v3, align 8
  %t7 = load i64, ptr %v3, align 8
  %t8 = sext i32 1 to i64
  %t9 = add i64 %t7, %t8
  store i64 %t9, ptr %v3, align 8
  %t10 = load i64, ptr %v3, align 8
  ret i64 %t10
}

define double @mix(i32 %p0, float %p1) {
entry:
  %v1 = alloca i32, align 4
  %v2 = alloca float, align 4
  %v3 = alloca double, align 8
  store i32 %p0, ptr %v1, align 4
  store float %p1, ptr %v2, align 4
  %t1 = load i32, ptr %v1, align 4
  %t2 = sitofp i32 %t1 to float
  %t3 = load float, ptr %v2, align 4
  %t4 = fadd float %t2, %t3
  %t5 = fpext float %t4 to double
  store double %t5, ptr %v3, align 8
  %t6 = load double, ptr %v3, align 8
  %t7 = sitofp i32 10 to double
  %t8 = fcmp ogt double %t6, %t7
  br i1 %t8, label %L1, label %L3
L1:
  %t9 = load double, ptr %v3, align 8
  %t10 = sitofp i32 2 to double
  %t11 = fdiv double %t9, %t10
  store double %t11, ptr %v3, align 8
  br label %L3
L3:
  %t12 = load double, ptr %v3, align 8
  ret double %t12
}

define i32 @main() {
entry:
  %v1 = alloca i32, align 4
  %v2 = alloca i32, align 4
  store i32 0, ptr %v1, align 4
  store i32 0, ptr %v2, align 4
  br label %L1
L1:
  %t1 = load i32, ptr %v2, align 4
  %t2 = icmp slt i32 %t1, 4
  br i1 %t2, label %L2, label %L4
L2:
  %t3 = load i32, ptr %v2, align 4
  %t4 = trunc i32 %t3 to i16
  %t5 = trunc i32 3 to i8
  %t6 = call i64 @scale(i16 signext %t4, i8 zeroext %t5)
  %t7 = trunc i64 %t6 to i32
  %t8 = load i32, ptr %v1, align 4
  %t9 = add i32 %t8, %t7
  store i32 %t9, ptr %v1, align 4
  br label %L3
L3:
  %t10 = load i32, ptr %v2, align 4
  %t11 = add i32 %t10, 1
  store i32 %t11, ptr %v2, align 4
  br label %L1
L4:
  br label %L5
L5:
  %t12 = load i32, ptr %v1, align 4
  %t13 = icmp sgt i32 %t12, 20
  br i1 %t13, label %L6, label %L7
L6:
  %t14 = load i32, ptr %v1, align 4
  %t15 = sub i32 %t14, 1
  store i32 %t15, ptr %v1, align 4
  br label %L5
L7:
  %t16 = load i32, ptr %v1, align 4
  %t17 = srem i32 %t16, 10
  %t18 = add i32 48, %t17
  %t19 = call i32 @putchar(i32 %t18)
  %t20 = call i32 @putchar(i32 10)
  %t21 = load i32, ptr %v1, align 4
  %t22 = call double @mix(i32 %t21, float 0x3FF8000000000000)
  %t23 = fcmp ogt double %t22, 0x4014000000000000
  br i1 %t23, label %L8, label %L9
L8:
  br label %L10
L9:
  br label %L10
L10:
  %t24 = phi i32 [ 0, %L8 ], [ 1, %L9 ]
  ret i32 %t24
}

