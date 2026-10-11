export type Protocol = 'anthropic' | 'openai' | 'generic-chat-completion-api';
export const protocols: Record<Protocol, string> = { anthropic: 'Anthropic Messages', openai: 'OpenAI Responses', 'generic-chat-completion-api': 'Chat Completions' };
export interface Provider { id: string; name: string; baseUrl: string; protocol: Protocol; credential: { mode: 'environment' | 'direct'; value: string }; modelListUrl: string | null; extraHeaders: Record<string,string> }
export interface ModelEntry { id:string; providerId:string; apiModelId:string; droidSelectorId:string|null; displayName:string; manufacturer:string|null; protocolOverride:Protocol|null; enabled:boolean; maxOutputTokens:number|null; imageInput:boolean; extraArgs:Record<string,unknown> }
export interface Catalog { providers:Provider[]; models:ModelEntry[] }
export interface Workspace { revision:number; targetPath:string; catalog:Catalog }
