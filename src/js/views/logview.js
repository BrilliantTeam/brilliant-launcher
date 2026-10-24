import { api, listen, openUrl } from '../api.js';
import { $, clear, confirm, dialog, fail, fmtBytes, h, toast, guard, RECYCLE_NOTE } from '../ui.js';
import { isRunning, on, selectedInstance, store } from '../store.js';

const MAX_LINES = 4000;
const FILE_LINES = 30000;
const LIVE = '';
const KINDS = { crash: '當機報告', log: '遊戲日誌', jvm: 'Java 崩潰紀錄', other: '其他紀錄檔' };
const logs = new Map();
let shown = null;
let source = LIVE;
let files = [];
let fileText = '';
let hits = [];
let hitIndex = -1;

const bufferOf = (id) => {
    if (!logs.has(id)) logs.set(id, []);
    return logs.get(id);
};

export const recentLog = (id) => (logs.get(id) ?? []).map((entry) => entry.line);

export function clearRecentLog(id) {
    logs.set(id, []);
    if (shown === id && source === LIVE) render();
}

function levelOf(line, stderr) {
    if (/\/(ERROR|FATAL)\]|\bFATAL\b|^\s*(Caused by|Exception in thread)|^\s+at [\w$.]+\(|\b\w+(Exception|Error):/.test(line)) return 'err';
    if (/\/WARN\]|\bWARN(ING)?\b/.test(line)) return 'warn';
    if (/\/(DEBUG|TRACE)\]/.test(line)) return 'dim';
    return stderr ? 'err' : '';
}

function mask(text) {
    if (!store.settings?.streamerMode) return text;
    let out = text
        .replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g, '<已隱藏>')
        .replace(/[\w.+-]+@[\w-]+\.[\w.-]+/g, '<已隱藏>')
        .replace(/[A-Za-z]:[\\/]Users[\\/][^\\/\s]+/gi, 'C:\\Users\\<使用者>')
        .replace(/\/(home|Users)\/[^/\s]+/g, '/$1/<使用者>');
    for (const account of store.accounts ?? []) {
        if ((account.name ?? '').length >= 3) out = out.split(account.name).join('<玩家>');
    }
    return out;
}

const lineEl = (line, stderr = false) => h('div', { class: levelOf(line, stderr) }, mask(line) || ' ');

function instanceIds() {
    const ids = store.instances.map((i) => i.id);
    for (const id of logs.keys()) if (!ids.includes(id)) ids.push(id);
    return ids;
}

function renderPickers() {
    const picker = $('#log-instance');
    clear(picker).append(...instanceIds().map((id) => {
        const name = store.instances.find((i) => i.id === id)?.name ?? id;
        return h('option', { value: id, selected: id === shown }, `${name}${isRunning(id) ? '（執行中）' : ''}`);
    }));
    const groups = Object.entries(KINDS).map(([kind, label]) => {
        const list = files.filter((f) => f.kind === kind);
        return list.length ? h('optgroup', { label }, ...list.map((f) => h('option', { value: f.path, selected: f.path === source },
            `${f.path}（${fmtBytes(f.size)}・${new Date(f.modified * 1000).toLocaleString('zh-TW', { hour12: false })}）`))) : null;
    });
    clear($('#log-source')).append(h('option', { value: LIVE, selected: source === LIVE }, '本次遊戲輸出（即時）'), ...groups.filter(Boolean));
    const live = source === LIVE;
    $('#btn-clear-log').hidden = !live;
    $('#btn-log-delete').hidden = live;
    $('#log-follow').closest('label').hidden = !live;
}

function render() {
    const box = $('#console');
    const fragment = document.createDocumentFragment();
    if (source === LIVE) {
        for (const { line, stderr } of logs.get(shown) ?? []) fragment.append(lineEl(line, stderr));
        if (!fragment.childNodes.length) fragment.append(h('div', { class: 'log-note' }, '這個實例目前沒有即時輸出。開始遊戲後會顯示在這裡；以前的紀錄可以從上方選擇紀錄檔查看。'));
    } else {
        const lines = fileText.split(/\r?\n/);
        if (lines.length > FILE_LINES) fragment.append(h('div', { class: 'log-note' }, `檔案很長，只顯示最後 ${FILE_LINES} 行（共 ${lines.length} 行）。完整內容請用「開啟資料夾」查看。`));
        for (const line of lines.slice(-FILE_LINES)) fragment.append(lineEl(line));
    }
    box.replaceChildren(fragment);
    box.scrollTop = source === LIVE || source.startsWith('logs/') ? box.scrollHeight : 0;
    search();
}

async function loadFiles() {
    files = shown ? await api.log_files({ id: shown }).catch(() => []) : [];
    if (source !== LIVE && !files.some((f) => f.path === source)) source = LIVE;
    renderPickers();
}

async function showSource(next) {
    source = next;
    fileText = '';
    if (source !== LIVE) {
        try {
            fileText = await api.read_log({ id: shown, path: source });
        } catch (error) {
            fail(error);
            source = LIVE;
        }
    }
    renderPickers();
    render();
}

async function showInstance(id) {
    shown = id;
    source = LIVE;
    await loadFiles();
    render();
}

const currentText = () => mask(source === LIVE ? recentLog(shown).join('\n') : fileText);

function search() {
    const query = $('#log-search').value.trim().toLowerCase();
    const box = $('#console');
    for (const el of box.querySelectorAll('.hit')) el.classList.remove('hit', 'current');
    hits = query ? [...box.children].filter((el) => el.textContent.toLowerCase().includes(query)) : [];
    for (const el of hits) el.classList.add('hit');
    hitIndex = -1;
    $('#log-hits').textContent = query ? (hits.length ? `${hits.length} 筆` : '找不到') : '';
}

function jump(step) {
    if (!hits.length) return search();
    hits[hitIndex]?.classList.remove('current');
    hitIndex = (hitIndex + step + hits.length) % hits.length;
    hits[hitIndex].classList.add('current');
    hits[hitIndex].scrollIntoView({ block: 'center' });
    $('#log-hits').textContent = `${hitIndex + 1} / ${hits.length}`;
}

async function uploadCurrent() {
    const text = currentText();
    if (!text.trim()) return toast('目前沒有內容可以上傳', 'warn');
    if (!(await confirm('要把這份紀錄上傳到 mclo.gs 嗎？\n上傳後任何拿到連結的人都能看到內容，適合貼給開發者或管理員協助除錯。\n啟動器會先遮掉登入權杖與電腦使用者名稱，mclo.gs 也會遮掉 IP 等敏感資訊。', '上傳'))) return;
    try {
        const url = await api.upload_log({ content: text });
        await navigator.clipboard.writeText(url).catch(() => {});
        await dialog({
            title: '已上傳',
            content: h('div', {}, h('p', {}, '連結已複製到剪貼簿：'), h('p', {}, h('a', { href: '#', onclick: (e) => { e.preventDefault(); openUrl(url); } }, url))),
            buttons: [{ label: '關閉', value: null }]
        });
    } catch (error) {
        fail(error);
    }
}

async function deleteFiles(list, label) {
    if (!list.length) return toast('沒有可以清除的紀錄檔');
    const size = list.reduce((sum, f) => sum + f.size, 0);
    if (!(await confirm(`${label}\n共 ${list.length} 個檔案（${fmtBytes(size)}），${RECYCLE_NOTE}`, '刪除'))) return;
    try {
        const removed = await api.delete_logs({ id: shown, pathsToDelete: list.map((f) => f.path) });
        toast(`已刪除 ${removed} 個紀錄檔`);
    } catch (error) {
        fail(error);
    }
    source = LIVE;
    await loadFiles();
    render();
}

export function initLog() {
    const box = $('#console');

    let pending = [];
    const flush = () => {
        const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
        const fresh = pending.filter((entry) => entry.instance === shown);
        pending = [];
        if (source !== LIVE || !fresh.length) return;
        box.querySelector('.log-note')?.remove();
        const fragment = document.createDocumentFragment();
        for (const { line, stderr } of fresh.slice(-MAX_LINES)) fragment.append(lineEl(line, stderr));
        box.append(fragment);
        for (let extra = box.childElementCount - MAX_LINES; extra > 0; extra--) box.firstElementChild.remove();
        if ($('#log-follow').checked && atBottom) box.scrollTop = box.scrollHeight;
    };
    listen('launcher://log', (entry) => {
        const isNew = !logs.has(entry.instance);
        const buffer = bufferOf(entry.instance);
        buffer.push({ line: entry.line, stderr: entry.stderr });
        if (buffer.length > MAX_LINES * 1.25) buffer.splice(0, buffer.length - MAX_LINES);
        if (shown === null || (isNew && source === LIVE && !logs.get(shown)?.length)) return showInstance(entry.instance);
        if (isNew) renderPickers();
        if (!pending.length) requestAnimationFrame(flush);
        pending.push(entry);
    });
    on('instances', () => {
        if (shown === null && store.instances.length) showInstance(selectedInstance()?.id ?? store.instances[0].id);
        else renderPickers();
    });

    const follow = $('#log-follow');
    const toBottom = () => requestAnimationFrame(() => { box.scrollTop = box.scrollHeight; });
    addEventListener('hashchange', () => {
        if (location.hash !== '#log') return;
        loadFiles();
        if (follow.checked && source === LIVE) toBottom();
    });
    follow.addEventListener('change', () => { if (follow.checked) toBottom(); });
    $('#log-wrap').addEventListener('change', (e) => box.classList.toggle('nowrap', !e.target.checked));
    $('#log-color').addEventListener('change', (e) => box.classList.toggle('plain', !e.target.checked));

    $('#log-instance').addEventListener('change', (e) => showInstance(e.target.value));
    $('#log-source').addEventListener('change', (e) => showSource(e.target.value));
    $('#btn-clear-log').addEventListener('click', () => {
        if (shown !== null) logs.set(shown, []);
        render();
    });
    $('#btn-log-reload').addEventListener('click', guard(async () => {
        await loadFiles();
        await showSource(source);
    }));
    $('#btn-log-copy').addEventListener('click', guard(async () => {
        await navigator.clipboard.writeText(currentText());
        toast('已複製到剪貼簿');
    }));
    $('#btn-log-upload').addEventListener('click', guard(uploadCurrent));
    $('#btn-log-delete').addEventListener('click', guard(() => deleteFiles(files.filter((f) => f.path === source), `要刪除「${source}」嗎？`)));
    $('#btn-log-clean').addEventListener('click', guard(() => deleteFiles(
        files.filter((f) => f.path.toLowerCase() !== 'logs/latest.log'),
        '要清除這個實例的舊紀錄檔嗎？（保留 logs/latest.log，其餘的日誌、當機報告與 Java 崩潰紀錄都會刪除）')));
    $('#btn-open-logs').addEventListener('click', guard(async () => {
        if (!shown) return;
        const sub = source.startsWith('crash-reports/') ? 'crash-reports' : source && !source.includes('/') ? '' : 'logs';
        await api.open_instance_folder({ id: shown, sub }).then((path) => api.open_path({ path })).catch(fail);
    }));

    const input = $('#log-search');
    let timer = 0;
    input.addEventListener('input', () => { clearTimeout(timer); timer = setTimeout(search, 200); });
    input.addEventListener('keydown', (e) => {
        if (e.key !== 'Enter') return;
        e.preventDefault();
        jump(e.shiftKey ? -1 : 1);
    });
    $('#btn-log-next').addEventListener('click', () => jump(1));
    $('#btn-log-prev').addEventListener('click', () => jump(-1));
    $('#btn-log-bottom').addEventListener('click', toBottom);

    if (store.instances.length) showInstance(selectedInstance()?.id ?? store.instances[0].id);
    else renderPickers();
}
