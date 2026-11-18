source_filename = "04_gnu.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

define i32 @classify(i32 %p0) {
entry:
  %v1 = alloca i32, align 4
  store i32 %p0, ptr %v1, align 4
  %t1 = load i32, ptr %v1, align 4
  switch i32 %t1, label %L3 [
    i32 97, label %L1
    i32 98, label %L1
    i32 99, label %L1
    i32 100, label %L1
    i32 101, label %L1
    i32 102, label %L1
    i32 103, label %L1
    i32 104, label %L1
    i32 105, label %L1
    i32 106, label %L1
    i32 107, label %L1
    i32 108, label %L1
    i32 109, label %L1
    i32 110, label %L1
    i32 111, label %L1
    i32 112, label %L1
    i32 113, label %L1
    i32 114, label %L1
    i32 115, label %L1
    i32 116, label %L1
    i32 117, label %L1
    i32 118, label %L1
    i32 119, label %L1
    i32 120, label %L1
    i32 121, label %L1
    i32 122, label %L1
    i32 48, label %L2
    i32 49, label %L2
    i32 50, label %L2
    i32 51, label %L2
    i32 52, label %L2
    i32 53, label %L2
    i32 54, label %L2
    i32 55, label %L2
    i32 56, label %L2
    i32 57, label %L2
  ]
L4:
  br label %L1
L1:
  ret i32 1
L2:
  ret i32 2
L3:
  ret i32 0
}

define i32 @main() {
entry:
  %v1 = alloca i64, align 8
  %v2 = alloca i32, align 4
  %v3 = alloca i32, align 4
  %se1.1 = alloca i32, align 4
  %t1 = sext i32 3 to i64
  store i64 %t1, ptr %v1, align 8
  %t2 = load i64, ptr %v1, align 8
  %t3 = trunc i64 %t2 to i32
  store i32 %t3, ptr %se1.1, align 4
  %t4 = load i32, ptr %se1.1, align 4
  %t5 = mul i32 %t4, 2
  store i32 %t5, ptr %v2, align 4
  %t6 = icmp ne i32 0, 0
  br i1 %t6, label %L1, label %L2
L1:
  br label %L3
L2:
  %t7 = load i32, ptr %v2, align 4
  br label %L3
L3:
  %t8 = phi i32 [ 0, %L1 ], [ %t7, %L2 ]
  store i32 %t8, ptr %v3, align 4
  %t9 = call i32 @classify(i32 113)
  %t10 = call i32 @classify(i32 53)
  %t11 = add i32 %t9, %t10
  %t12 = sub i32 %t11, 3
  %t13 = load i32, ptr %v3, align 4
  %t14 = sub i32 %t13, 6
  %t15 = add i32 %t12, %t14
  ret i32 %t15
}

