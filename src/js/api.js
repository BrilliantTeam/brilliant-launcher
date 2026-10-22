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
