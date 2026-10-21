export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

export function rangeFill(el) {
    const min = Number(el.min) || 0;
    const max = Number(el.max) || 100;
    el.style.setProperty('--fill', `${((Number(el.value) - min) / Math.max(1, max - min)) * 100}%`);
}

export function guard(fn) {
    return function (event) {
        const el = event.currentTarget;
        if (el.hasAttribute('data-pending')) return;
        const result = fn.call(this, event);
        if (!result || typeof result.then !== 'function') return result;
        el.setAttribute('data-pending', '');
        el.setAttribute('aria-busy', 'true');
        return Promise.resolve(result).finally(() => {
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
                h('button', { type: 'button', class: 'mc', onclick: () => close(btn.value) }, btn.label)
            );
        }
        dlg.showModal();
        const first = foot.querySelector('button');
        if (first) first.focus();
    });
}

export const dismissDialog = (value) => closeOpenDialog?.(value);

export async function confirm(message, okLabel = '確定') {
    const answer = await dialog({
        title: '確認',
        content: h('p', {}, message),
        buttons: [
            { label: '取消', value: false },
            { label: okLabel, value: true }
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
