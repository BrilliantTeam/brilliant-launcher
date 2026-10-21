import { api } from './api.js';
import { clear, dialog, fail, fmtBytes, h, toast } from './ui.js';
import { selectedAccount, store } from './store.js';
import { gpuName } from './services.js';
import { recentLog } from './views/logview.js';

const LEVELS = [
    ['basic', '基本', '錯誤摘要與最後 80 行日誌，通常就夠管理員判斷'],
    ['standard', '標準', '再加完整遊戲日誌、當機報告、模組清單與系統資訊'],
    ['full', '完整', '再加啟動器日誌、錯誤紀錄、效能紀錄與設定，最容易查出原因']
];
const ITEMS = [
    ['game_log', '完整遊戲日誌（最多 1 MB）', 'standard'],
    ['crash_report', '當機報告', 'standard'],
    ['mods', '模組清單', 'standard'],
    ['system', '系統資訊（作業系統、記憶體、Java、顯示卡）', 'standard'],
    ['launcher_log', '啟動器日誌', 'full'],
    ['panic', '啟動器錯誤紀錄', 'full'],
    ['native', 'Java 崩潰紀錄', 'full'],
    ['perf', '效能紀錄（FPS、卡頓）', 'full'],
    ['settings', '啟動器與實例設定（已去除敏感資料）', 'full']
];
const RANK = { basic: 0, standard: 1, full: 2 };

export function scrub(text, accounts = store.accounts || []) {
    let out = String(text ?? '');
    for (const account of accounts) {
        for (const secret of [account.name, account.uuid, account.uuid?.replace(/-/g, '')].filter((v) => v && v.length > 2)) out = out.split(secret).join('<player>');
    }
    return out
        .replace(/([A-Za-z]:[\\/]+Users[\\/]+)[^\\/\s"']+/gi, '$1<user>')
        .replace(/\/(home|Users)\/[^/\s"']+/g, '/$1/<user>')
        .replace(/eyJ[\w-]+\.[\w-]+\.[\w-]+/g, '<token>')
        .replace(/(--accessToken\s+)\S+/g, '$1<token>');
}

export function buildSummary(kind, instance, extra, clean) {
    return {
        kind,
        mc: instance?.mcVersion || '',
        loader: instance ? `${instance.loader} ${instance.loaderVersion || ''}`.trim() : '',
        general: !!instance?.general,
        code: extra.code ?? null,
        message: clean(extra.message),
        findings: (extra.findings || []).map((f) => clean(f)),
        exception: clean(extra.exception),
        log: recentLog(instance?.id).slice(-80).map((line) => clean(line))
    };
}

export async function reportDialog(kind, instance, extra = {}) {
    const account = selectedAccount();
    let level = 'basic';
    const boxes = ITEMS.map(([key, label, min]) => ({ key, min, input: h('input', { type: 'checkbox' }), label }));
    const levelInputs = LEVELS.map(([value, label, hint]) => {
        const input = h('input', { type: 'radio', name: 'report-level', value, checked: value === level });
        input.addEventListener('change', () => {
            level = value;
            for (const b of boxes) b.input.checked = RANK[b.min] <= RANK[level];
            clear(preview);
        });
        return h('label', { class: 'report-level' }, input, h('span', {}, h('b', {}, label), h('small', {}, hint)));
    });
    const withName = h('input', { type: 'checkbox' });
    const contact = h('input', { type: 'text', maxlength: 100, placeholder: '聯絡方式（選填，例如 Discord 名稱）' });
    const description = h('textarea', { maxlength: 1000, rows: 3, placeholder: '發生了什麼事？（選填，例如：進炒飯服就閃退）' });
    const preview = h('div', { class: 'report-preview' });

    const chosen = () => boxes.filter((b) => b.input.checked).map((b) => b.key);
    const clean = (text) => scrub(text, (store.accounts || []).filter((a) => !withName.checked || a.id !== account?.id));
    const gather = async () => {
        const sections = await api.collect_report({ id: instance?.id ?? '', items: chosen(), since: extra.since ?? 0 });
        const gpu = gpuName();
        return sections.map((s) => ({ ...s, text: clean(s.key === 'system' && gpu ? `${s.text}\n顯示卡：${gpu}` : s.text) }));
    };
    const showPreview = async () => {
        clear(preview).append(h('div', { class: 'hint' }, '整理中⋯'));
        const sections = await gather().catch((error) => { fail(error); return []; });
        const summary = buildSummary(kind, instance, extra, clean);
        const base = [summary.message, ...summary.findings, summary.exception, ...summary.log].filter(Boolean).join('\n');
        const items = [{ title: '錯誤摘要與最後 80 行日誌', text: base }, ...sections];
        const total = items.reduce((n, s) => n + new Blob([s.text]).size, 0);
        clear(preview).append(h('div', { class: 'hint' }, `共 ${items.length} 項，約 ${fmtBytes(total)}。以下就是會送出的內容，帳號權杖與電腦使用者名稱已遮蔽。`),
            ...items.map((s) => h('details', {}, h('summary', {}, `${s.title}（${fmtBytes(new Blob([s.text]).size)}）`), h('pre', {}, s.text.slice(0, 20000) + (s.text.length > 20000 ? '\n⋯（預覽只顯示前 20000 字，送出時是完整內容）' : '')))));
    };

    const value = await dialog({
        title: '回報給管理員',
        wide: true,
        content: h('div', { class: 'report' },
            h('p', { class: 'hint' }, '選擇要提供多少資料。越詳細越容易找出原因；送出前可以先預覽，所有內容都只有輝煌管理員看得到，30 天後自動刪除。'),
            h('div', { class: 'report-levels' }, ...levelInputs),
            h('div', { class: 'report-items' }, h('label', { class: 'report-item fixed' }, h('input', { type: 'checkbox', checked: true, disabled: true }), '錯誤摘要與最後 80 行日誌'),
                ...boxes.map((b) => h('label', { class: 'report-item' }, b.input, b.label))),
            h('label', { class: 'report-item' }, withName, `附上我的遊戲名稱${account ? `（${account.name}）` : ''}，方便管理員聯絡我`),
            contact,
            description,
            h('button', { type: 'button', class: 'mc', onclick: showPreview }, '預覽要送出的內容'),
            preview),
        buttons: [{ label: '取消', value: null }, { label: '送出回報', value: 'send' }]
    });
    if (value !== 'send') return;
    toast('正在整理並送出回報⋯');
    try {
        const sections = await gather();
        const body = {
            ...buildSummary(kind, instance, extra, clean),
            level,
            sections,
            description: description.value.trim(),
            contact: contact.value.trim(),
            player: withName.checked ? account?.name ?? '' : ''
        };
        const result = await api.send_report({ body });
        toast(`已回報給管理員${result?.id ? `（回報編號 ${result.id.slice(0, 8)}）` : ''}，謝謝你！`);
    } catch (error) {
        fail(error);
    }
}
