export type Protocol = "anthropic" | "openai" | "generic-chat-completion-api";
export const protocols: Record<Protocol, string> = {
  anthropic: "Anthropic Messages",
  openai: "OpenAI Responses",
  "generic-chat-completion-api": "Chat Completions",
};
export interface Provider {
  id: string;
  name: string;
  baseUrl: string;
  protocol: Protocol;
  credential: { mode: "environment" | "direct"; value: string };
  modelListUrl: string | null;
  extraHeaders: Record<string, string>;
}
export interface ModelEntry {
  id: string;
  providerId: string;
  apiModelId: string;
  droidSelectorId: string | null;
  displayName: string;
  manufacturer: string | null;
  protocolOverride: Protocol | null;
  enabled: boolean;
  maxOutputTokens: number | null;
  imageInput: boolean;
  extraArgs: Record<string, unknown>;
}
export interface Catalog {
  providers: Provider[];
  models: ModelEntry[];
}
export interface Workspace {
  revision: number;
  recoveryPending?: boolean;
  backupRetentionDays?: number;
  usageSettings?: { path: string; retentionDays: number };
  targetPath: string;
  catalog: Catalog;
  scheme?: Scheme;
  appliedScheme?: Scheme;
  appliedCatalog?: Catalog;
  baseline?: { document: Record<string, unknown>; fingerprint: unknown } | null;
  bindings?: { modelId: string; original: unknown }[];
}
export interface Scheme {
  source: "official" | "api" | null;
  official: { model: string | null; reasoning: string | null };
  api: { modelId: string | null; reasoning: string | null };
}
export interface ObservedModel {
  id: string;
  name: string;
  source: "official" | "api";
  reasoning: string[];
  defaultReasoning: string | null;
}
