// 页面快照：一次连接，同时产出三臂输入。
//   A 臂 → text   （document.body.innerText 纯文本）
//   B 臂 → png    （Page.captureScreenshot）
//   C 臂 → pkg    （本象包：identity + facts 对象图 + 任务约束声明）
import { start } from './cdp.mjs';

// 通用事实抽取：把任意页面的可操作对象 + 关键数据对象拉成结构化对象图。
const EXTRACT = `(() => {
  const $ = (id) => document.getElementById(id);
  const t = (el) => el ? el.textContent.trim() : null;

  const inputs = [...document.querySelectorAll('input')].map(el => ({
    id: el.id || null, name: el.name || null, type: el.type || 'text',
    value: el.value, placeholder: el.placeholder || null,
    stableId: el.id || (el.hasAttribute('data-idx') ? 'qty-' + el.dataset.idx : null)
  }));
  const buttons = [...document.querySelectorAll('button')].map(el => ({
    id: el.id || null, text: t(el), disabled: el.disabled, visible: el.offsetParent !== null
  }));
  const links = [...document.querySelectorAll('a')].map(el => ({
    id: el.id || null, text: t(el), href: el.getAttribute('href')
  }));
  const selects = [...document.querySelectorAll('select')].map(el => ({
    id: el.id || null, value: el.value,
    options: [...el.options].map(o => o.value + ':' + o.textContent.trim())
  }));
  // 带 id 的关键数据对象（小计/总价/指标/确认信息等）——排除容器，避免整页文本重复
  const CONTAINERS = new Set(['form','table','div','tbody','thead','ul','ol','nav','header','footer','main']);
  const keyValues = [...document.querySelectorAll('[id]')]
    .filter(el => {
      const tag = el.tagName.toLowerCase();
      return t(el) && !CONTAINERS.has(tag) && !['input','button','select','a'].includes(tag);
    })
    .map(el => ({ id: el.id, text: t(el), visible: el.offsetParent !== null }));
  // 容器（form/div/section/table…）的可见性状态——不取文本（避免整页重复），只取可见性
  const containers = [...document.querySelectorAll('form,div,section,table,nav,main')]
    .filter(el => el.id)
    .map(el => ({ id: el.id, tag: el.tagName.toLowerCase(), visible: el.offsetParent !== null }));

  return {
    identity: { url: location.href, title: document.title, hash: location.hash },
    facts: { inputs, buttons, links, selects, keyValues, containers },
  };
})()`;

export async function snapshotFixture(fixture) {
  const { goto } = await start();
  const tab = await goto(fixture);

  const state = await tab.evalJson(`(() => {
    const base = ${EXTRACT};
    base.text = document.body.innerText;
    return base;
  })()`);

  const shot = await tab.send('Page.captureScreenshot', { format: 'png' });
  state.pngBase64 = shot.data;
  await tab.close();
  return state;
}
