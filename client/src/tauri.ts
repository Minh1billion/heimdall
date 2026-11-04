import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

window.heimdall = {
  close: () => {
    void invoke("window_close");
  },
  toggleMaximize: () => invoke<boolean>("window_toggle_maximize"),
  isMaximized: () => invoke<boolean>("window_is_maximized"),
  saveChartPng: (name: string, data: number[]) => invoke<string>("save_chart_png", { name, data }),
  invoke: (method: string, path: string, body?: unknown) =>
    invoke<{ status: number; body: unknown }>("api_call", { method, path, body }),
};

document.addEventListener("click", (event) => {
  if (event.defaultPrevented || !(event.target instanceof Element)) return;
  const anchor = event.target.closest("a");
  const href = anchor?.getAttribute("href") ?? "";
  if (anchor && (href.startsWith("http://") || href.startsWith("https://"))) {
    event.preventDefault();
    void openUrl(href);
  }
});
