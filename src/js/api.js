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
