// Gate 0 任务定义：每个任务 = 测试页 fixture + 问题集 + 本象包约束声明 + 判分真值。
// 「约束声明」是 C 臂（本象包）区别于 A/B 臂的差异化内容——把不变量显式告诉 agent。

export const TASKS = {
  task1: {
    fixture: 'order-form.html',
    title: '果蔬订单',
    questions: [
      { id: 'Q1', text: '这个页面是什么？请列出页面上的商品、各自单价和当前数量，以及当前总价。' },
      { id: 'Q2', text: '现在要买：苹果 2 个、香蕉 3 个、橙子 1 个。请说明需要在哪几行、各填多少数量；并计算填完后页面总价应显示为多少。' },
      { id: 'Q3', text: '填写完数量并点击「提交订单」后，如何确认提交真的成功了？请列出你应该检查的所有检查点，以及成功时每个检查点应满足的条件。' },
    ],
    // 不变量声明：C 臂注入，A/B 臂不注入（它们要自己从原始表示里推断）
    constraints: [
      { id: 'c:line-0', kind: 'derived', relation: 'kv:line-total-0.text == 10 × in:qty-0.value', note: '苹果行小计 = 单价10 × 数量' },
      { id: 'c:line-1', kind: 'derived', relation: 'kv:line-total-1.text == 5 × in:qty-1.value', note: '香蕉行小计 = 单价5 × 数量' },
      { id: 'c:line-2', kind: 'derived', relation: 'kv:line-total-2.text == 8 × in:qty-2.value', note: '橙子行小计 = 单价8 × 数量' },
      { id: 'c:grand', kind: 'derived', relation: 'kv:grand-total.text == kv:line-total-0.text + kv:line-total-1.text + kv:line-total-2.text', note: '总价 = 三行小计之和' },
      { id: 'c:qty-valid', kind: 'invariant', relation: '0 ≤ in:qty-*.value ≤ 99 且为整数', note: '数量校验规则，违反则提交被拒绝' },
      { id: 'c:success', kind: 'invariant', relation: '提交成功 ⟺ page.hash == "#confirmed" ∧ kv:confirmation.visible == true ∧ kv:order-form.visible == false ∧ kv:confirm-count.text == 已选件数(2+3+1=3) ∧ kv:confirm-total.text == kv:grand-total.text(43元)', note: '提交成功的充要条件' },
    ],
    // 判分真值：judge 对照 agent 回答逐点打勾
    groundTruth: {
      Q1: ['标题为果蔬订单', '苹果单价10', '香蕉单价5', '橙子单价8', '当前数量均为0', '当前总价为0元'],
      Q2: ['苹果行填2', '香蕉行填3', '橙子行填1', '总价为43元'],
      Q3: ['URL hash 变为 #confirmed', '确认区可见', '确认数量显示3', '确认金额显示43元', '表单被隐藏'],
    },
  },

  task2: {
    fixture: 'operable-page.html',
    title: '数据看板',
    questions: [
      { id: 'Q1', text: '请列出这个页面里所有可操作对象（输入框/按钮/链接/下拉框），每个标注：类型、唯一标识（id）、用途。' },
    ],
    constraints: [
      { id: 'c:operable', kind: 'invariant', relation: '可操作对象 = select + input + button + a 的并集，缺一不算完整', note: '枚举完整性检查' },
    ],
    groundTruth: {
      Q1: ['select 日期范围 date-range', 'input 关键字 kw-input', 'button 应用筛选 filter-btn', 'a 导出报表 export-link', 'button 刷新 refresh-btn', 'button 显示明细 toggle-detail-btn'],
    },
  },
};
