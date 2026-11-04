import { Copy, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function WindowBar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const sync = () => {
      void window.heimdall.isMaximized().then(setMaximized);
    };
    sync();
    const unlisten = getCurrentWindow().onResized(sync);
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  return (
    <div
      className="window-bar"
      data-tauri-drag-region
      onDoubleClick={(event) => {
        if (event.target === event.currentTarget) void window.heimdall.toggleMaximize().then(setMaximized);
      }}
    >
      <button
        type="button"
        className="window-btn"
        aria-label={maximized ? "Restore" : "Maximize"}
        title={maximized ? "Restore" : "Maximize"}
        onClick={() => void window.heimdall.toggleMaximize().then(setMaximized)}
      >
        {maximized ? <Copy size={13} /> : <Square size={13} />}
      </button>
      <button
        type="button"
        className="window-btn window-close"
        aria-label="Close"
        title="Close (Heimdall keeps running in the tray)"
        onClick={() => window.heimdall.close()}
      >
        <X size={16} />
      </button>
    </div>
  );
}
