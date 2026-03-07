spec: task
name: "退款功能"
inherits: project
tags: [payment, refund]
---

## 意图

为支付网关添加退款功能，支持全额和部分退款。
退款需要管理员权限，且必须在原始交易后 90 天内发起。

## 约束

- 退款金额不得超过原始交易金额
- 退款操作需要管理员权限
- 退款必须在原交易后 90 天内发起
- 退款状态机: pending -> processing -> completed | failed
- 响应应该快速返回

## 验收标准

场景: 全额退款
  测试: test_full_refund_flow
  假设 存在一笔金额为 "100.00" 元的已完成交易 "TXN-001"
  并且 当前用户具有管理员权限
  当 用户对 "TXN-001" 发起全额退款
  那么 退款状态变为 "processing"
  并且 原始交易状态变为 "refunding"

场景: 部分退款
  测试: test_partial_refund_flow
  假设 存在一笔金额为 "100.00" 元的已完成交易 "TXN-002"
  当 用户对 "TXN-002" 发起 "30.00" 元的部分退款
  那么 剩余可退金额为 "70.00" 元
  并且 允许后续再次部分退款

场景: 退款拒绝 - 超期
  测试: test_refund_rejects_expired_transaction
  假设 存在一笔 91 天前完成的交易 "TXN-003"
  当 用户对 "TXN-003" 发起退款
  那么 系统拒绝退款
  并且 返回错误信息包含 "超过退款期限"

场景: 退款拒绝 - 金额超限
  测试: test_refund_rejects_amount_exceeding_original
  假设 存在一笔金额为 "100.00" 元的已完成交易 "TXN-004"
  当 用户对 "TXN-004" 发起 "150.00" 元的退款
  那么 系统拒绝退款
  并且 返回错误码 "REFUND_EXCEEDS_ORIGINAL"

## 排除范围

- 登录功能
- 密码重置
- 第三方支付对接
