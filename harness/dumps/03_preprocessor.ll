source_filename = "03_preprocessor.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

@.str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@label = internal global ptr @.str, align 8

define i32 @pick(i32 %p0) {
entry:
  %v1 = alloca i32, align 4
  store i32 %p0, ptr %v1, align 4
  %t1 = load i32, ptr %v1, align 4
  switch i32 %t1, label %L4 [
    i32 0, label %L1
    i32 5, label %L2
    i32 6, label %L3
  ]
L6:
  br label %L1
L1:
  ret i32 114
L2:
  br label %L3
L3:
  %t2 = load i32, ptr %v1, align 4
  %t3 = load i32, ptr %v1, align 4
  %t4 = mul i32 %t2, %t3
  ret i32 %t4
L4:
  ret i32 1
L5:
  ret i32 0
}

define i32 @main() {
entry:
  %v1 = alloca i64, align 8
  %v2 = alloca x86_fp80, align 16
  %v3 = alloca i64, align 8
  %v4 = alloca i32, align 4
  %t1 = add i64 16, 8
  store i64 %t1, ptr %v1, align 8
  store x86_fp80 0xK3FFFA000000000000000, ptr %v2, align 16
  store i64 18446744073709551615, ptr %v3, align 8
  %t2 = add i32 1, 120
  %t3 = sub i32 %t2, 120
  store i32 %t3, ptr %v4, align 4
  %t4 = call i32 @pick(i32 6)
  %t5 = sub i32 %t4, 36
  %t6 = load i64, ptr %v1, align 8
  %t7 = sext i32 24 to i64
  %t8 = sub i64 %t6, %t7
  %t9 = trunc i64 %t8 to i32
  %t10 = add i32 %t5, %t9
  %t11 = load x86_fp80, ptr %v2, align 16
  %t12 = sitofp i32 1 to x86_fp80
  %t13 = fcmp olt x86_fp80 %t11, %t12
  %t14 = zext i1 %t13 to i32
  %t15 = add i32 %t10, %t14
  %t16 = load i64, ptr %v3, align 8
  %t17 = sext i32 63 to i64
  %t18 = lshr i64 %t16, %t17
  %t19 = trunc i64 %t18 to i32
  %t20 = add i32 %t15, %t19
  %t21 = load i32, ptr %v4, align 4
  %t22 = sub i32 %t20, %t21
  %t23 = add i32 %t22, 16
  %t24 = sub i32 %t23, 16
  ret i32 %t24
}

