import { api, listen } from '../api.js';
import { $, fail, h } from '../ui.js';

const MAX_LINES = 4000;
const RECENT_LINES = 1500;
let recent = [];

export const recentLog = () => recent;
export const clearRecentLog = () => { recent = []; };

export function initLog() {
    const box = $('#console');

    listen('launcher://log', ({ line, stderr }) => {
        recent.push(line);
        if (recent.length > RECENT_LINES) recent = recent.slice(-RECENT_LINES);
        const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
        box.append(h('div', { class: stderr ? 'err' : '' }, line));
        while (box.childElementCount > MAX_LINES) box.firstElementChild.remove();
        if ($('#log-follow').checked && atBottom) box.scrollTop = box.scrollHeight;
    });

    $('#btn-clear-log').addEventListener('click', () => box.replaceChildren());
    $('#btn-open-logs').addEventListener('click', async () => {
        const paths = await api.launcher_paths();
        const logs = paths.find(([label]) => label === '日誌');
        if (logs) await api.open_path({ path: logs[1] }).catch(fail);
    });
}
