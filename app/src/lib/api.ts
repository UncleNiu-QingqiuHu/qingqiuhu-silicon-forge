import { invoke } from "@tauri-apps/api/core";
import type {
  ConvertOptions,
  ConvertOutcome,
  Feature,
  ImageProbe,
  SplitOutcome,
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
};

export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
