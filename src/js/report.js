import { api } from './api.js';
import { clear, dialog, fail, fmtBytes, h, toast } from './ui.js';
import { accountFor, emit, on, saveSettings, store } from './store.js';
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
    const account = accountFor(instance);
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
            h('p', { class: 'hint' }, '選擇要提供多少資料。越詳細越容易找出原因；送出前可以先預覽，所有內容都只有輝煌管理員看得到，處理完 30 天後自動刪除。送出後可以在「日誌 → 我的回報」查看進度與管理員的回覆。'),
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
        if (result?.id && result?.token) {
            const cause = body.findings[0] || body.message.split('\n')[0] || (kind === 'crash' ? '遊戲異常結束' : '無法啟動遊戲');
            await saveSettings({ myReports: [{ id: result.id, token: result.token, at: Date.now(), cause, alerted: 0, seen: 0 }, ...(store.settings.myReports || [])].slice(0, 20) }).catch(() => {});
        }
        toast(`已回報給管理員${result?.id ? `（回報編號 ${result.id.slice(0, 8)}）` : ''}，謝謝你！可以在「日誌 → 我的回報」查看處理進度`);
    } catch (error) {
        fail(error);
    }
}

const replies = (r) => r.messages.filter((m) => m.kind === 'admin').length;
const when = (at) => new Date(at).toLocaleString('zh-TW', { hour12: false });

export async function refreshReports() {
    const mine = store.settings?.myReports || [];
    if (!mine.length) return [];
    const { reports } = await api.report_post({ action: 'status', body: { items: mine.map(({ id, token }) => ({ id, token })) } });
    const byId = new Map(reports.map((r) => [r.id, r]));
    const fresh = mine.filter((m) => byId.has(m.id) && replies(byId.get(m.id)) > (m.alerted || 0));
    if (fresh.length) {
        toast(`管理員回覆了你的錯誤回報，到「日誌 → 我的回報」查看`);
        if (store.settings?.notifications) api.notify_user({ title: '輝煌管理員回覆了你的回報', body: byId.get(fresh[0].id).messages.filter((m) => m.kind === 'admin').at(-1).text.slice(0, 120) }).catch(() => {});
    }
    const recent = (m) => Date.now() - (m.at || 0) < 35 * 86400000;
    const kept = mine.filter((m) => byId.has(m.id) || recent(m)).map((m) => (byId.has(m.id) ? { ...m, cause: byId.get(m.id).cause, alerted: replies(byId.get(m.id)) } : m));
    if (fresh.length || kept.length !== mine.length) await saveSettings({ myReports: kept }).catch(() => {});
    store.reports = reports;
    emit('reports');
    return reports;
}

function unread() {
    const byId = new Map((store.reports || []).map((r) => [r.id, r]));
    return (store.settings?.myReports || []).filter((m) => byId.has(m.id) && replies(byId.get(m.id)) > (m.seen || 0)).length;
}

function reportItem(mine, r) {
    const box = h('details', { class: 'my-report', open: r && replies(r) > (mine.seen || 0) });
    const render = (r) => {
        clear(box).append(h('summary', {},
            h('span', { class: `my-status ${r?.status || 'gone'}` }, r?.statusName || '無法取得狀態'),
            h('b', {}, r?.cause || mine.cause),
            h('small', {}, `#${mine.id.slice(0, 8)}｜${when(mine.at)}`)));
        if (!r) return box.append(h('p', { class: 'hint' }, '目前連不上官網，恢復連線後再看看。'));
        box.append(h('ul', { class: 'my-thread' }, ...(r.messages.length ? r.messages : [{ kind: 'status', text: '管理員還沒處理，收到回覆時會通知你。', at: r.at }])
            .map((m) => h('li', { class: m.kind }, h('small', {}, `${{ admin: '輝煌管理員', player: '你', status: '狀態' }[m.kind]}｜${when(m.at)}`), m.text))));
        const input = h('textarea', { rows: 2, maxlength: 1000, placeholder: r.open ? '補充說明或回覆管理員⋯' : '問題又出現了？在這裡回覆會重新開啟回報' });
        const send = h('button', { type: 'button', class: 'mc', onclick: async () => {
            if (!input.value.trim()) return;
            send.disabled = true;
            try {
                render(await api.report_post({ action: 'reply', body: { id: mine.id, token: mine.token, text: input.value.trim() } }));
                toast('已送出，管理員會收到通知');
            } catch (error) {
                send.disabled = false;
                fail(error);
            }
        } }, '送出回覆');
        box.append(h('div', { class: 'my-reply' }, input, send));
    };
    render(r);
    return box;
}

export async function myReportsDialog() {
    const list = h('div', { class: 'my-reports' }, h('div', { class: 'hint' }, '讀取中⋯'));
    const shown = dialog({ title: '我的回報', wide: true, content: list });
    const reports = await refreshReports().catch(() => null);
    const mine = store.settings?.myReports || [];
    const byId = new Map((reports || []).map((r) => [r.id, r]));
    clear(list).append(...(mine.length
        ? [h('p', { class: 'hint' }, '這裡是你用「回報給管理員」送出的回報。管理員回覆時會通知你；結案 30 天後會從這裡消失。'), ...mine.map((m) => reportItem(m, byId.get(m.id)))]
        : [h('p', { class: 'hint' }, '你還沒有送出過回報。遊戲啟動失敗或異常結束時，可以按「回報給管理員」把資料交給管理員處理。')]));
    if (reports) {
        await saveSettings({ myReports: mine.map((m) => ({ ...m, seen: byId.has(m.id) ? replies(byId.get(m.id)) : m.seen })) }).catch(() => {});
        emit('reports');
    }
    return shown;
}

export function initReports() {
    const button = document.getElementById('btn-my-reports');
    button.addEventListener('click', () => myReportsDialog().catch(fail));
    const label = () => {
        const n = unread();
        button.textContent = n ? `我的回報（${n} 則新回覆）` : '我的回報';
        button.classList.toggle('primary', n > 0);
    };
    on('reports', label);
    on('settings', label);
    const poll = () => { if (store.online !== false) refreshReports().catch(() => {}); };
    setTimeout(poll, 12_000);
    setInterval(poll, 10 * 60 * 1000);
    on('network', poll);
}
