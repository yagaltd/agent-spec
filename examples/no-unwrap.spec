spec: task
name: "代码质量检查"
tags: [quality]
---

## 意图

确保代码库不使用危险的方法调用。

## 约束

### 禁止做

- 生产代码中禁止使用 `.unwrap()`
- 禁止使用 `panic!` 宏
- 禁止使用 `todo!` 宏

## 验收标准

场景: 无 unwrap 调用
  测试: test_no_unwrap_calls_exist
  假设 代码库已编译通过
  当 扫描所有源代码文件
  那么 不应存在 .unwrap() 调用
