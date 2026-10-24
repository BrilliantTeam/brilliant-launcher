export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

export function rangeFill(el) {
    const min = Number(el.min) || 0;
    const max = Number(el.max) || 100;
    el.style.setProperty('--fill', `${((Number(el.value) - min) / Math.max(1, max - min)) * 100}%`);
}

let pendingWork = 0;
export const workPending = () => pendingWork > 0;

export function guard(fn) {
    return function (event) {
        const el = event.currentTarget;
        if (el.hasAttribute('data-pending')) return;
        const result = fn.call(this, event);
        if (!result || typeof result.then !== 'function') return result;
        el.setAttribute('data-pending', '');
        el.setAttribute('aria-busy', 'true');
        pendingWork++;
        return Promise.resolve(result).finally(() => {
            pendingWork--;
            el.removeAttribute('data-pending');
            el.removeAttribute('aria-busy');
        });
    };
}

export function clampNumber(el, show = false) {
    const min = el.min === '' ? -Infinity : Number(el.min);
    const max = el.max === '' ? Infinity : Number(el.max);
    const value = Math.min(max, Math.max(min, Math.round(Number(el.value) || 0)));
    if (show && el.value !== '' && String(value) !== el.value) el.value = String(value);
    return value;
}

addEventListener('change', (e) => {
    if (e.target instanceof HTMLInputElement && e.target.type === 'number') clampNumber(e.target, true);
}, true);

export function h(tag, props = {}, ...children) {
    const el = document.createElement(tag);
    if (tag === 'input' || tag === 'textarea') el.setAttribute('autocomplete', 'off');
    for (const [key, value] of Object.entries(props || {})) {
        if (value === null || value === undefined || value === false) continue;
        if (key === 'onclick' && typeof value === 'function') el.addEventListener('click', guard(value));
        else if (key.startsWith('on') && typeof value === 'function') el.addEventListener(key.slice(2), value);
        else if (key === 'html') el.innerHTML = value;
        else if (key in el && key !== 'list' && typeof value !== 'string') el[key] = value;
        else el.setAttribute(key, value === true ? '' : value);
    }
    for (const child of children.flat()) {
        if (child === null || child === undefined || child === false) continue;
        el.append(child instanceof Node ? child : document.createTextNode(String(child)));
    }
    return el;
}

for (const proto of [Element.prototype, DocumentFragment.prototype]) {
    for (const name of ['append', 'prepend', 'replaceChildren']) {
        const native = proto[name];
        proto[name] = function (...nodes) {
            return native.apply(this, nodes.filter((n) => n !== null && n !== undefined && n !== false));
        };
    }
}

const HISTORY_MAX = 8;
const historyKey = (input) => `brilliant.history.${input.id}`;
const hasHistory = (el) => el instanceof HTMLInputElement && el.type === 'search' && !!el.id;
let historyBox = null;
let historyActive = -1;

function readHistory(input) {
    try {
        const list = JSON.parse(localStorage.getItem(historyKey(input)));
        return Array.isArray(list) ? list.filter((v) => typeof v === 'string') : [];
    } catch {
        return [];
    }
}

function writeHistory(input, list) {
    try {
        localStorage.setItem(historyKey(input), JSON.stringify(list.slice(0, HISTORY_MAX)));
    } catch {}
}

function rememberSearch(input) {
    const value = input.value.trim();
    if (value) writeHistory(input, [value, ...readHistory(input).filter((v) => v !== value)]);
}

function closeHistory() {
    historyBox?.remove();
    historyBox = null;
    historyActive = -1;
}

function pickHistory(input, value) {
    input.value = value;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    rememberSearch(input);
    closeHistory();
}

function showHistory(input) {
    closeHistory();
    const query = input.value.trim().toLowerCase();
    const items = readHistory(input).filter((v) => v.toLowerCase() !== query && (!query || v.toLowerCase().includes(query)));
    if (!items.length) return;
    const rect = input.getBoundingClientRect();
    historyBox = h('div', { class: 'search-history', role: 'listbox', 'aria-label': '最近搜尋', style: `left:${rect.left}px;top:${rect.bottom + 4}px;width:${rect.width}px` },
        h('div', { class: 'search-history-head' }, '最近搜尋'),
        ...items.map((value) => h('div', { class: 'search-history-item', role: 'option', 'data-value': value },
            h('button', { type: 'button', class: 'history-pick', tabindex: '-1', onmousedown: (e) => { e.preventDefault(); pickHistory(input, value); } }, value),
            h('button', {
                type: 'button',
                class: 'history-forget',
                tabindex: '-1',
                title: '移除這筆紀錄',
                'aria-label': `移除「${value}」這筆搜尋紀錄`,
                onmousedown: (e) => {
                    e.preventDefault();
                    writeHistory(input, readHistory(input).filter((v) => v !== value));
                    showHistory(input);
                }
            }, '×'))));
    document.body.append(historyBox);
}

function moveHistory(step) {
    const items = [...(historyBox?.querySelectorAll('.search-history-item') ?? [])];
    if (!items.length) return;
    historyActive = (historyActive + step + items.length) % items.length;
    items.forEach((el, i) => el.setAttribute('aria-selected', String(i === historyActive)));
}

for (const el of document.querySelectorAll('input, textarea')) el.setAttribute('autocomplete', 'off');

addEventListener('focusin', (e) => {
    const el = e.target;
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) el.setAttribute('autocomplete', 'off');
    if (hasHistory(el)) showHistory(el);
});
addEventListener('focusout', (e) => { if (hasHistory(e.target)) { rememberSearch(e.target); closeHistory(); } });
addEventListener('input', (e) => { if (hasHistory(e.target) && document.activeElement === e.target) showHistory(e.target); });
addEventListener('scroll', closeHistory, true);
addEventListener('resize', closeHistory);
addEventListener('keydown', (e) => {
    if (!hasHistory(e.target)) return;
    if (historyBox && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
        e.preventDefault();
        moveHistory(e.key === 'ArrowDown' ? 1 : -1);
    } else if (e.key === 'Enter') {
        const active = historyBox?.querySelectorAll('.search-history-item')[historyActive];
        if (active) {
            e.preventDefault();
            pickHistory(e.target, active.dataset.value);
        } else {
            rememberSearch(e.target);
            closeHistory();
        }
    } else if (e.key === 'Escape' && historyBox) {
        e.stopPropagation();
        closeHistory();
    }
}, true);

export function clear(el) {
    el.replaceChildren();
    return el;
}

const TOAST_MS = 6000;

export function toast(message, kind = '') {
    const box = h('div', { class: `toast ${kind}` }, String(message));
    $('#toasts').append(box);
    setTimeout(() => {
        box.classList.add('out');
        setTimeout(() => box.remove(), 250);
    }, kind === 'error' ? TOAST_MS * 2 : TOAST_MS);
}

export function fail(error) {
    console.error(error);
    toast(typeof error === 'string' ? error : (error?.message ?? String(error)), 'error');
}

let closeOpenDialog = null;

export const RECYCLE_NOTE = '會移到啟動器的回收資料夾，30 天內都能從「關於 → 回收資料夾」找回。';

addEventListener('unhandledrejection', (e) => {
    e.preventDefault();
    fail(e.reason);
});

const DESTRUCTIVE = /刪除|移除|清除|解散|覆蓋|登出|重設|停用|強制/;

export function dialog({ title, content, wide = false, buttons = [{ label: '關閉', value: null }] }) {
    const dlg = $('#dlg');
    dlg.classList.toggle('wide', wide);
    const body = clear($('#dlg-body'));
    const foot = clear($('#dlg-foot'));
    if (title) body.append(h('h2', {}, title));
    body.append(content);

    if (closeOpenDialog) closeOpenDialog(null);
    return new Promise((resolve) => {
        const close = (value) => {
            if (closeOpenDialog === close) closeOpenDialog = null;
            dlg.close();
            dlg.removeEventListener('cancel', onCancel);
            resolve(value);
        };
        closeOpenDialog = close;
        const onCancel = () => close(null);
        dlg.addEventListener('cancel', onCancel, { once: true });

        for (const btn of buttons) {
            foot.append(
                h('button', { type: 'button', class: `mc ${btn.kind ?? (DESTRUCTIVE.test(btn.label) && btn.value ? 'danger' : '')}`.trim(), onclick: () => close(btn.value) }, btn.label)
            );
        }
        dlg.showModal();
        const field = body.querySelector('input[type="text"]:not([disabled])');
        (field ?? foot.querySelector('button'))?.focus();
    });
}

addEventListener('keydown', (e) => {
    const input = e.target;
    if (e.key !== 'Enter' || e.isComposing || e.keyCode === 229 || !(input instanceof HTMLInputElement) || !['text', 'number'].includes(input.type)) return;
    const dlg = input.closest('#dlg');
    const buttons = dlg?.open ? [...dlg.querySelectorAll('#dlg-foot button')] : [];
    const primary = buttons.at(-1);
    if (buttons.length < 2 || primary.classList.contains('danger')) return;
    e.preventDefault();
    primary.click();
});

export const dismissDialog = (value) => closeOpenDialog?.(value);

const fileZones = new WeakMap();

export function acceptFiles(el, type, handler, { multiple = false } = {}) {
    el.classList.add('file-drop');
    fileZones.set(el, { type, handler, multiple });
    return el;
}

export function fileZoneAt(x, y) {
    let el = document.elementFromPoint(x, y)?.closest('.file-drop');
    while (el && !fileZones.has(el)) el = el.parentElement?.closest('.file-drop');
    return el ? { el, ...fileZones.get(el) } : null;
}

export const modalOpen = () => [...document.querySelectorAll('dialog[open]')].length > 0;

export async function confirm(message, okLabel = '確定') {
    const answer = await dialog({
        title: '確認',
        content: h('p', {}, message),
        buttons: [
            { label: '取消', value: false },
            { label: okLabel, value: true, kind: DESTRUCTIVE.test(okLabel) ? 'danger' : 'primary' }
        ]
    });
    return answer === true;
}

export function fmtBytes(bytes) {
    if (!bytes) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
    return `${(bytes / 1024 ** i).toFixed(i ? 1 : 0)} ${units[i]}`;
}

export function fmtDuration(seconds) {
    if (!seconds) return '尚未遊玩';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return hours ? `已遊玩 ${hours} 小時 ${minutes} 分` : `已遊玩 ${minutes} 分`;
}

export function fmtDate(unixSeconds) {
    if (!unixSeconds) return '從未';
    return new Date(unixSeconds * 1000).toLocaleString('zh-TW', { hour12: false });
}
