export type FeatureStatus = "committed" | "conditional" | "deferred" | "out";

export interface Feature {
  id: string;
  name: string;
  module: string;
  status: FeatureStatus;
  milestone: string | null;
  placeholder: string | null;
  blocked_by: string[];
  /** 是否已在本版本交付；committed 但未交付 = 开发中 */
  delivered: boolean;
  available: boolean;
}

export interface CrossRef {
  cell: string;
  formula: string;
  targets: string[];
}

export interface SheetInfo {
  name: string;
  state: string;
  rows: number;
  cols: number;
  has_chart: boolean;
  cross_refs: CrossRef[];
  truncated: boolean;
}

export interface WorkbookScan {
  path: string;
  sheet_count: number;
  sheets: SheetInfo[];
  defined_name_refs: CrossRef[];
}

export interface SplitOutcome {
  sheet: string;
  path: string;
  bytes: number;
}

export interface ImageProbe {
  width: number;
  height: number;
  format: string;
  bytes: number;
}

export interface ConvertOptions {
  target_format: string;
  quality: number;
  max_edge: number | null;
  scale_percent: number | null;
  background: string | null;
}

export interface ConvertOutcome {
  source: string;
  output: string;
  bytes: number;
  width: number;
  height: number;
  note: string | null;
}

export type CrossRefPolicy = "keep" | "to_value" | "abort";

export interface ExportOptions {
  format: string;
  title: string;
  page_numbers: boolean;
  out_dir: string | null;
}

export interface ExportOutcome {
  path: string;
  bytes: number;
  format: string;
  note: string | null;
}
