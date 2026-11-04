import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  ConvertOptions,
  ConvertOutcome,
  ExportOptions,
  ExportOutcome,
  Feature,
  ImageProbe,
  SplitOutcome,
  UpdateChannel,
  UpdateInfo,
  WorkbookScan,
  CrossRefPolicy,
} from "./types";

/** 所有后端调用的唯一出口，便于统一处理错误与后续加日志。 */
export const api = {
  features: () => invoke<Feature[]>("get_features"),

  excelScan: (path: string) => invoke<WorkbookScan>("excel_scan", { path }),

  excelSplit: (
    path: string,
    sheets: string[],
    outDir: string | null,
    policy: CrossRefPolicy
  ) => invoke<SplitOutcome[]>("excel_split", { path, sheets, outDir, policy }),

  imageProbe: (src: string) => invoke<ImageProbe>("image_probe", { src }),

  imageConvert: (src: string, outDir: string | null, options: ConvertOptions) =>
    invoke<ConvertOutcome>("image_convert", { src, outDir, options }),

  hideToTray: () => invoke<void>("hide_to_tray"),

  confirmExit: () => invoke<void>("confirm_exit"),

  setCloseBehavior: (toTray: boolean) =>
    invoke<void>("set_close_behavior", { toTray }),

  getCloseBehavior: () => invoke<boolean>("get_close_behavior"),

  getAutostart: () => invoke<boolean>("get_autostart"),

  setAutostart: (enable: boolean) => invoke<void>("set_autostart", { enable }),

  readTextFile: (path: string) => invoke<string>("read_text_file", { path }),

  writeTextFile: (path: string, content: string) =>
    invoke<void>("write_text_file", { path, content }),

  mdPreview: (markdown: string) => invoke<string>("md_preview", { markdown }),

  mdExport: (markdown: string, sourcePath: string | null, options: ExportOptions) =>
    invoke<ExportOutcome>("md_export", { markdown, sourcePath, options }),

  appVersion: () => invoke<string>("app_version"),

  getAppConfig: () => invoke<AppConfig>("get_app_config"),

  getConfigFatal: () => invoke<string | null>("get_config_fatal"),

  checkUpdate: () => invoke<UpdateInfo>("check_update"),

  installUpdate: () => invoke<void>("install_update"),

  setUpdateChannel: (channel: UpdateChannel) =>
    invoke<void>("set_update_channel", { channel }),

  setUpdateEndpoint: (endpoint: string) =>
    invoke<void>("set_update_endpoint", { endpoint }),

  setCheckOnStart: (enable: boolean) => invoke<void>("set_check_on_start", { enable }),
};

export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
