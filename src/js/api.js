const tauri = window.__TAURI__;

export const api = new Proxy(
    {},
    { get: (_, name) => (args) => tauri.core.invoke(name, args) }
);

export const listen = (event, handler) => tauri.event.listen(event, (e) => handler(e.payload));

export const openUrl = (url) => tauri.opener.openUrl(url);

export const pickFiles = (options) => tauri.dialog.open(options);

export const appWindow = () => tauri.window.getCurrentWindow();

export const fileSrc = (path) => tauri.core.convertFileSrc(path);

export const setZoom = (scale) => tauri.webview.getCurrentWebview().setZoom(scale);

export const appVersion = () => tauri.app.getVersion();
