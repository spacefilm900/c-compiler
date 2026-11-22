source_filename = "02_records.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.point = type { i8, i32, i32 }
%struct.flags = type { [ 1 x i32 ] }
%union.word = type { [ 1 x i32 ] }
@.str = private unnamed_addr constant [6 x i8] c"point\00", align 1
@origin = global %struct.point { i8 111, i32 0, i32 0 }, align 4
@where = global ptr @origin, align 8
@table = global [4 x i32] [i32 1, i32 2, i32 0, i32 4], align 16
@third = global ptr getelementptr (i8, ptr @table, i64 8), align 8
@name = global ptr @.str, align 8

define i32 @area(ptr %p0, i32 %p1) {
entry:
  %v1 = alloca ptr, align 8
  %v2 = alloca %struct.flags, align 4
  %v3 = alloca %union.word, align 4
  store ptr %p0, ptr %v1, align 8
  store i32 %p1, ptr %v2, align 4
  %t1 = getelementptr inbounds i8, ptr %v3, i64 0
  store i32 0, ptr %t1, align 4
  %t2 = getelementptr inbounds i8, ptr %v3, i64 0
  %t3 = sext i32 1 to i64
  %t4 = getelementptr inbounds i8, ptr %t2, i64 %t3
  %t5 = getelementptr inbounds i8, ptr %v2, i64 1
  %t6 = load i8, ptr %t5, align 1
  store i8 %t6, ptr %t4, align 1
  %t7 = getelementptr inbounds i8, ptr %v2, i64 0
  %t8 = load i32, ptr %t7, align 4
  %t9 = lshr i32 %t8, 1
  %t10 = and i32 %t9, 7
  %t11 = add i32 %t10, 2
  %t12 = load i32, ptr %t7, align 4
  %t13 = and i32 %t11, 7
  %t14 = shl i32 %t13, 1
  %t15 = and i32 %t12, 4294967281
  %t16 = or i32 %t15, %t14
  store i32 %t16, ptr %t7, align 4
  %t17 = load ptr, ptr %v1, align 8
  %t18 = getelementptr inbounds %struct.point, ptr %t17, i32 0, i32 1
  %t19 = load i32, ptr %t18, align 4
  %t20 = load ptr, ptr %v1, align 8
  %t21 = getelementptr inbounds %struct.point, ptr %t20, i32 0, i32 2
  %t22 = load i32, ptr %t21, align 4
  %t23 = mul i32 %t19, %t22
  %t24 = getelementptr inbounds i8, ptr %v2, i64 0
  %t25 = load i32, ptr %t24, align 4
  %t26 = lshr i32 %t25, 1
  %t27 = and i32 %t26, 7
  %t28 = add i32 %t23, %t27
  %t29 = getelementptr inbounds i8, ptr %v2, i64 0
  %t30 = load i32, ptr %t29, align 4
  %t31 = lshr i32 %t30, 4
  %t32 = and i32 %t31, 15
  %t33 = shl i32 %t32, 28
  %t34 = ashr i32 %t33, 28
  %t35 = add i32 %t28, %t34
  %t36 = getelementptr inbounds i8, ptr %v3, i64 0
  %t37 = load i32, ptr %t36, align 4
  %t38 = add i32 %t35, %t37
  ret i32 %t38
}

define i32 @main() {
entry:
  %v1 = alloca %struct.flags, align 4
  %v2 = alloca %struct.point, align 4
  %t1 = sub i32 0, 3
  %t2 = trunc i32 4 to i8
  store %struct.flags zeroinitializer, ptr %v1, align 4
  %t3 = load i32, ptr %v1, align 4
  %t4 = and i32 1, 1
  %t5 = and i32 %t3, 4294967294
  %t6 = or i32 %t5, %t4
  store i32 %t6, ptr %v1, align 4
  %t7 = load i32, ptr %v1, align 4
  %t8 = and i32 2, 7
  %t9 = shl i32 %t8, 1
  %t10 = and i32 %t7, 4294967281
  %t11 = or i32 %t10, %t9
  store i32 %t11, ptr %v1, align 4
  %t12 = load i32, ptr %v1, align 4
  %t13 = and i32 %t1, 15
  %t14 = shl i32 %t13, 4
  %t15 = and i32 %t12, 4294967055
  %t16 = or i32 %t15, %t14
  store i32 %t16, ptr %v1, align 4
  %t17 = getelementptr inbounds i8, ptr %v1, i64 1
  store i8 %t2, ptr %t17, align 1
  store %struct.point zeroinitializer, ptr %v2, align 4
  %t18 = getelementptr inbounds i8, ptr %v2, i64 4
  store i32 3, ptr %t18, align 4
  %t19 = getelementptr inbounds i8, ptr %v2, i64 8
  store i32 5, ptr %t19, align 4
  %t20 = load i32, ptr %v1, align 4
  %t21 = call i32 @area(ptr %v2, i32 %t20)
  %t22 = sub i32 %t21, 1043
  %t23 = sext i32 3 to i64
  %t24 = getelementptr inbounds i32, ptr @table, i64 %t23
  %t25 = load i32, ptr %t24, align 4
  %t26 = add i32 %t22, %t25
  %t27 = load ptr, ptr @third, align 8
  %t28 = load i32, ptr %t27, align 4
  %t29 = sub i32 %t26, %t28
  %t30 = load ptr, ptr @where, align 8
  %t31 = icmp eq ptr %t30, @origin
  %t32 = zext i1 %t31 to i32
  %t33 = sub i32 %t29, %t32
  %t34 = load ptr, ptr @name, align 8
  %t35 = sext i32 0 to i64
  %t36 = getelementptr inbounds i8, ptr %t34, i64 %t35
  %t37 = load i8, ptr %t36, align 1
  %t38 = sext i8 %t37 to i32
  %t39 = sub i32 %t38, 112
  %t40 = sub i32 %t33, %t39
  ret i32 %t40
}

