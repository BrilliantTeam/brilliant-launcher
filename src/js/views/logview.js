import { api, listen } from '../api.js';
import { $, fail, h, guard } from '../ui.js';

const MAX_LINES = 4000;
const RECENT_LINES = 1500;
let recent = [];

export const recentLog = () => recent;
export const clearRecentLog = () => { recent = []; };

export function initLog() {
    const box = $('#console');

    let pending = [];
    const flush = () => {
        const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
        const fragment = document.createDocumentFragment();
        for (const { line, stderr } of pending.slice(-MAX_LINES)) fragment.append(h('div', { class: stderr ? 'err' : '' }, line));
        pending = [];
        box.append(fragment);
        for (let extra = box.childElementCount - MAX_LINES; extra > 0; extra--) box.firstElementChild.remove();
        if ($('#log-follow').checked && atBottom) box.scrollTop = box.scrollHeight;
    };
    listen('launcher://log', (entry) => {
        recent.push(entry.line);
        if (recent.length > RECENT_LINES * 2) recent = recent.slice(-RECENT_LINES);
        if (!pending.length) requestAnimationFrame(flush);
        pending.push(entry);
    });

    $('#btn-clear-log').addEventListener('click', () => box.replaceChildren());
    $('#btn-open-logs').addEventListener('click', guard(async () => {
        const paths = await api.launcher_paths();
        const logs = paths.find(([label]) => label === '日誌');
        if (logs) await api.open_path({ path: logs[1] }).catch(fail);
    }));
}
