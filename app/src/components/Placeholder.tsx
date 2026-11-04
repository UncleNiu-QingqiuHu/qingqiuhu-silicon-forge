import type { Feature } from "../lib/types";

/**
 * 未交付功能的统一呈现（PRD 8.4）。
 * 不显示假控件：要么明确标注状态，要么不放输入框。
 */
export default function Placeholder({ feature }: { feature?: Feature }) {
  const status = feature?.status;
  const delivered = feature?.delivered ?? false;

  let tag = "待排期";
  let warn = true;

  if (status === "deferred") {
    tag = "已延后";
  } else if (status === "conditional") {
    tag = feature?.milestone ? `待排期 · ${feature.milestone}` : "待排期";
  } else if (status === "committed" && !delivered) {
    tag = "开发中";
    warn = false;
  }

  const text =
    feature?.placeholder ??
    (status === "committed" && !delivered
      ? `${feature?.name ?? "该模块"}计划于 ${feature?.milestone ?? "后续版本"} 交付。`
      : "功能正在建设中，敬请期待。");

  return (
    <div className="placeholder">
      <div className={`tag ${warn ? "warn" : ""}`}>{tag}</div>
      <h3>{feature?.name ?? "该模块尚未开放"}</h3>
      <p>{text}</p>
      {feature && feature.blocked_by.length > 0 && (
        <div className="blockers">前置条件：{feature.blocked_by.join(" / ")}</div>
      )}
    </div>
  );
}
