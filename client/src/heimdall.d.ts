export {};

declare global {
  interface Window {
    heimdall: {
      close(): void;
      toggleMaximize(): Promise<boolean>;
      isMaximized(): Promise<boolean>;
      saveChartPng(name: string, data: number[]): Promise<string>;
      invoke(method: string, path: string, body?: unknown): Promise<{ status: number; body: unknown }>;
    };
  }
}
