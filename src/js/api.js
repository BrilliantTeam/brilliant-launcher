const tauri = window.__TAURI__;

const NETWORK_ERROR = /error sending request|dns error|tcp connect|connection (?:refused|reset)|timed out|network is unreachable|沒有網路|連不上/i;

export const api = new Proxy(
    {},
    {
        get: (_, name) => (args) => tauri.core.invoke(name, args).catch((error) => {
            if (name !== 'net_status' && NETWORK_ERROR.test(String(error?.message ?? error))) dispatchEvent(new Event('brilliant:net-error'));
            throw error;
        })
    }
);

export const listen = (event, handler) => tauri.event.listen(event, (e) => handler(e.payload));

export const openUrl = (url) => tauri.opener.openUrl(url);

export const pickFiles = (options) => tauri.dialog.open(options);

const RASTER = ['png', 'jpg', 'jpeg', 'webp', 'gif'];

export const FILE_TYPES = {
    skin: { name: '皮膚 PNG', extensions: ['png'] },
    cover: { name: '封面圖片', extensions: [...RASTER, 'svg'] },
    icon: { name: '圖示圖片', extensions: [...RASTER, 'svg'] },
    background: { name: '背景圖片或影片', extensions: [...RASTER, 'mp4', 'webm'] },
    music: { name: '背景音樂', extensions: ['mp3', 'ogg', 'wav', 'm4a', 'flac', 'opus'] },
    font: { name: '字型檔', extensions: ['ttf', 'otf', 'woff', 'woff2'] },
    world: { name: '世界壓縮檔', extensions: ['zip'] },
    mrpack: { name: 'Modrinth 模組包', extensions: ['mrpack'] },
    config: { name: '輝煌啟動器設定', extensions: ['zip'] },
    content: { name: '模組、資源包或光影', extensions: ['jar', 'zip'] },
    pack: { name: '資源包或光影包', extensions: ['zip'] }
};

export const extOf = (path) => (/\.([^.\\/]+)$/.exec(String(path)) || [])[1]?.toLowerCase() ?? '';
export const isType = (path, type) => FILE_TYPES[type].extensions.includes(extOf(path));
export const typeLabel = (type) => `${FILE_TYPES[type].name}（${FILE_TYPES[type].extensions.map((e) => e.toUpperCase()).join('、')}）`;

export function pickFile(type, { multiple = false } = {}) {
    return pickFiles({ multiple, filters: [{ name: FILE_TYPES[type].name, extensions: FILE_TYPES[type].extensions }] });
}

export const appWindow = () => tauri.window.getCurrentWindow();

export const fileSrc = (path) => tauri.core.convertFileSrc(path);

export const setZoom = (scale) => tauri.webview.getCurrentWebview().setZoom(scale);

export const appVersion = () => tauri.app.getVersion();

export function watchFolders(subs, target, onChange) {
    let key;
    let last;
    const check = async () => {
        if (document.hidden) return;
        const id = target();
        if (id === undefined) {
            key = undefined;
            return;
        }
        const stamp = await api.dir_stamp({ subs, id }).catch(() => null);
        if (stamp === null || target() !== id) return;
        const changed = key === (id ?? '*') && last !== stamp;
        key = id ?? '*';
        last = stamp;
        if (changed) onChange();
    };
    const timer = setInterval(check, 2000);
    addEventListener('focus', check);
    document.addEventListener('visibilitychange', check);
    check();
    return () => {
        clearInterval(timer);
        removeEventListener('focus', check);
        document.removeEventListener('visibilitychange', check);
    };
}
