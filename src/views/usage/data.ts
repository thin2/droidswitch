export interface TokenTotals {
  input: number;
  cacheRead: number;
  cacheWrite: number;
  output: number;
  thinking: number;
  unknownInput: number;
  unknownCacheRead: number;
  unknownCacheWrite: number;
  unknownOutput: number;
  unknownThinking: number;
}

export interface CurrencyTotal {
  currency: string;
  cost: string;
}

export interface Summary {
  tokens: TokenTotals;
  records: number;
  sessions: number;
  estimates: CurrencyTotal[];
  pricedRecords: number;
  billableRecords: number;
  partialRecords: number;
  unknownRecords: number;
  credits: number;
  creditsRecords: number;
  unknownSourceRecords: number;
}

export interface TrendBucket {
  start: number;
  records: number;
  tokens: TokenTotals;
  estimates: CurrencyTotal[];
}

export interface Group {
  key: string;
  label: string;
  tokens: TokenTotals;
  records: number;
  estimates: CurrencyTotal[];
  pricedRecords: number;
  unpricedRecords: number;
  unknownRecords: number;
}

export interface RecordRow {
  id: string;
  occurredAt: number;
  session: string;
  granularity: string;
  timeConfidence: string;
  source: string;
  provider: string | null;
  providerLabel: string;
  selector: string | null;
  apiModel: string | null;
  input: number | null;
  cacheRead: number | null;
  cacheWrite: number | null;
  output: number | null;
  thinking: number | null;
  requestId: string | null;
  attempt: number | null;
  cost: string | null;
  currency: string | null;
  complete: boolean;
}

export interface Coverage {
  ledgerSessions: number;
  ledgerTokens: number;
  ledgerUnknownFields: number;
  detailTokens: number;
  unallocatedTokens: number;
  detailRecords: number;
  requestRecords: number;
  baselineTokens: number;
  windowFrom: number | null;
  windowTo: number | null;
  endpointResolvedRecords: number;
  endpointUnresolvedRecords: number;
}

export interface StoredPrice {
  source?: string;
  id: string;
  providerId: string;

  apiModelId: string;
  currency: string;
  effectiveAt: number;
  input: string | null;
  cacheRead: string | null;
  cacheWrite: string | null;
  output: string | null;
  thinking: string | null;
}

export interface Page {
  summary: Summary;
  trends: TrendBucket[];
  unassigned: CurrencyTotal[];
  unassignedTokens: TokenTotals;
  providers: Group[];
  models: Group[];
  records: RecordRow[];
  totalRecords: number;
  coverage: Coverage;
  prices: StoredPrice[];
  providersFilter: (string | null)[];
  providerLabels: Record<string, string>;
  modelsFilter: string[];
}

export interface ScanReport {
  logFiles: number;
  logRequests: number;
  logUnpaired: number;
  logInconsistent: number;
  sessionFiles: number;
  sessions: number;
  snapshots: number;
  baselines: number;
  deltas: number;
  imported: number;
  duplicates: number;
  unsupported: number;
  malformed: number;
  unreadable: number;
  counterResets: number;
  updatedAt: number;
}

export interface UsageSettings {
  path: string;
  logsPath: string;
  exportsPath: string;
  retentionDays: number;
  autoScan: boolean;
}

export interface UsagePayload {
  settings: UsageSettings;
  notice: string | null;
  report: ScanReport | null;
  page: Page;
  scanning: boolean;
}

export interface Filter {
  from: number;
  to: number;
  source?: string;
  provider?: string;
  model?: string;
  granularity?: string;
  includeUnknownTime?: boolean;
  bucket?: string;
  offsetMinutes?: number;
}

export function formatTokens(value: number, unknownCount: number): string {
  if (value === 0 && unknownCount > 0) return "未知";
  const suffix = unknownCount > 0 ? "（部分缺失）" : "";
  return `${value.toLocaleString()}${suffix}`;
}

export function formatCost(estimates: CurrencyTotal[]): string {
  if (estimates.length === 0) return "未定价";
  return estimates
    .map(
      (e) =>
        `${e.currency} ${Number(e.cost) > 0 && Number(e.cost) < 0.0001 ? "<0.0001" : Number(e.cost).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 4 })}`,
    )
    .join(" · ");
}

export function tokenTotal(tokens: TokenTotals): number {
  return (
    tokens.input +
    tokens.cacheRead +
    tokens.cacheWrite +
    tokens.output +
    tokens.thinking
  );
}

export function csv(records: RecordRow[]) {
  const cell = (value: unknown) => {
    let s = value === null || value === undefined ? "未知" : String(value);
    if (/^[\s]*[=+\-@]/.test(s) || /^[\t\r]/.test(s)) s = "'" + s;
    return '"' + s.replaceAll('"', '""') + '"';
  };
  const fields = [
    "时间",
    "来源",
    "供应商",
    "API模型",
    "Droid选择器",
    "普通输入",
    "缓存读",
    "缓存写",
    "输出",
    "思考",
    "估算费用",
    "币种",
    "粒度",
    "时间置信度",
  ];
  return (
    "\ufeff" +
    [
      fields,
      ...records.map((r) => [
        new Date(r.occurredAt * 1000).toISOString(),
        r.source,
        r.providerLabel || "未知",
        r.apiModel,
        r.selector,
        r.input,
        r.cacheRead,
        r.cacheWrite,
        r.output,
        r.thinking,
        r.cost,
        r.currency,
        r.granularity,
        r.timeConfidence,
      ]),
    ]
      .map((row) => row.map(cell).join(","))
      .join("\r\n")
  );
}

export function download(text: string, name: string, type: string) {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
