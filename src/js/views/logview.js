import { api, listen } from '../api.js';
import { $, clear, fail, h, guard } from '../ui.js';
import { isRunning, on, store } from '../store.js';

const MAX_LINES = 4000;
const logs = new Map();
let shown = null;

const bufferOf = (id) => {
    if (!logs.has(id)) logs.set(id, []);
    return logs.get(id);
};

export const recentLog = (id) => (logs.get(id) ?? []).map((entry) => entry.line);

export function clearRecentLog(id) {
    logs.set(id, []);
    showLog(id);
}

function renderPicker() {
    const picker = $('#log-instance');
    const ids = [...logs.keys()];
    picker.hidden = ids.length < 2;
    clear(picker).append(...ids.map((id) => {
        const name = store.instances.find((i) => i.id === id)?.name ?? id;
        return h('option', { value: id, selected: id === shown }, `${name}${isRunning(id) ? '（執行中）' : ''}`);
    }));
}

function showLog(id) {
    shown = id;
    const box = $('#console');
    const fragment = document.createDocumentFragment();
    for (const { line, stderr } of logs.get(id) ?? []) fragment.append(h('div', { class: stderr ? 'err' : '' }, line));
    box.replaceChildren(fragment);
    box.scrollTop = box.scrollHeight;
    renderPicker();
}

export function initLog() {
    const box = $('#console');

    let pending = [];
    const flush = () => {
        const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
        const fresh = pending.filter((entry) => entry.instance === shown);
        pending = [];
        const fragment = document.createDocumentFragment();
        for (const { line, stderr } of fresh.slice(-MAX_LINES)) fragment.append(h('div', { class: stderr ? 'err' : '' }, line));
        box.append(fragment);
        for (let extra = box.childElementCount - MAX_LINES; extra > 0; extra--) box.firstElementChild.remove();
        if ($('#log-follow').checked && atBottom) box.scrollTop = box.scrollHeight;
    };
    listen('launcher://log', (entry) => {
        const isNew = !logs.has(entry.instance);
        const buffer = bufferOf(entry.instance);
        buffer.push({ line: entry.line, stderr: entry.stderr });
        if (buffer.length > MAX_LINES * 1.25) buffer.splice(0, buffer.length - MAX_LINES);
        if (shown === null) return showLog(entry.instance);
        if (isNew) renderPicker();
        if (!pending.length) requestAnimationFrame(flush);
        pending.push(entry);
    });
    on('instances', renderPicker);

    const follow = $('#log-follow');
    const toBottom = () => requestAnimationFrame(() => { box.scrollTop = box.scrollHeight; });
    addEventListener('hashchange', () => { if (location.hash === '#log' && follow.checked) toBottom(); });
    follow.addEventListener('change', () => { if (follow.checked) toBottom(); });

    $('#log-instance').addEventListener('change', (e) => showLog(e.target.value));
    $('#btn-clear-log').addEventListener('click', () => {
        if (shown !== null) logs.set(shown, []);
        box.replaceChildren();
    });
    $('#btn-open-logs').addEventListener('click', guard(async () => {
        const paths = await api.launcher_paths();
        const folder = paths.find(([label]) => label === '日誌');
        if (folder) await api.open_path({ path: folder[1] }).catch(fail);
    }));
}
