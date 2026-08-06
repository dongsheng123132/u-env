// 线 B 雏形：CDP 执行「填表 → 校验 → 提交 → 验证」完整动作链，
// 每个动作后重读状态，对照预期做 parity 校验。
import { start as startCDP } from './lib/cdp.mjs';

const { goto } = await startCDP();
const tab = await goto('order-form.html');

// --- 动作 1：填写三个商品数量 ---
await tab.send('Runtime.evaluate', {
  expression: `(() => {
    const setQty = (i, v) => {
      const el = document.querySelector('input.qty[data-idx="'+i+'"]');
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    };
    setQty(0, 2); setQty(1, 3); setQty(2, 1);
  })()`
});

// --- parity 1：总价应为 2*10 + 3*5 + 1*8 = 43 ---
let st = await tab.evalJson(`({
  grand: document.getElementById('grand-total').textContent,
  line0: document.getElementById('line-total-0').textContent,
  line1: document.getElementById('line-total-1').textContent,
  line2: document.getElementById('line-total-2').textContent
})`);
console.log('[parity 1] 填表后:', JSON.stringify(st));
console.log('[parity 1]', st.grand === '43 元' && st.line0 === '20 元' && st.line1 === '15 元' && st.line2 === '8 元'
  ? 'PASS ✅' : 'FAIL ❌');

// --- 动作 2：提交订单 ---
await tab.send('Runtime.evaluate', { expression: `document.getElementById('submit-btn').click()` });
await new Promise(r => setTimeout(r, 300));

// --- parity 2：跳转确认页 + 金额一致 ---
st = await tab.evalJson(`({
  hash: location.hash,
  confDisplay: document.getElementById('confirmation').style.display,
  confirmCount: document.getElementById('confirm-count').textContent,
  confirmTotal: document.getElementById('confirm-total').textContent,
  formDisplay: document.getElementById('order-form').style.display
})`);
console.log('[parity 2] 提交后:', JSON.stringify(st));
console.log('[parity 2]', st.hash === '#confirmed' && st.confDisplay === 'block' && st.confirmTotal === '43 元' && st.confirmCount === '3'
  ? 'PASS ✅' : 'FAIL ❌');

await tab.close();
console.log('done');
process.exit(0);
