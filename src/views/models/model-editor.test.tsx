import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,render,screen,waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {WorkspaceProvider} from '../../lib/workspace';
import {request} from '../../lib/api';
import {ModelEditor} from './ModelEditor';
import type {ModelEntry} from '../../lib/types';
vi.mock('../../lib/api',()=>({request:vi.fn(),errorText:(e:unknown)=>String(e)}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
const model:ModelEntry={id:'model-1',providerId:'p',apiModelId:'exact/id:1',droidSelectorId:null,displayName:'Original',manufacturer:null,protocolOverride:null,enabled:true,maxOutputTokens:100,imageInput:false,extraArgs:{}};
test('model-editor preserves API ID, clears optional overrides and rejects invalid JSON',async()=>{
 const mock=vi.mocked(request);mock.mockResolvedValue({revision:2,targetPath:'/test',catalog:{providers:[{id:'p',name:'Provider'}],models:[model]}});
 const user=userEvent.setup();const close=vi.fn();render(<WorkspaceProvider><ModelEditor model={model} onClose={close}/></WorkspaceProvider>);
 await screen.findByText('Provider');await user.clear(screen.getByLabelText('最大输出 Token'));await user.clear(screen.getByLabelText('显示名称'));await user.type(screen.getByLabelText('显示名称'),'Renamed');
 await user.clear(screen.getByLabelText('额外参数（JSON）'));await user.type(screen.getByLabelText('额外参数（JSON）'),'invalid');await user.click(screen.getByRole('button',{name:'保存草稿'}));expect(mock.mock.calls.some(([c])=>c==='save_model')).toBe(false);
 await user.clear(screen.getByLabelText('额外参数（JSON）'));await user.type(screen.getByLabelText('额外参数（JSON）'),'{{"temperature":0.2}');await user.click(screen.getByRole('button',{name:'保存草稿'}));
 await waitFor(()=>expect(mock).toHaveBeenCalledWith('save_model',expect.objectContaining({revision:2,model:expect.objectContaining({id:'model-1',apiModelId:'exact/id:1',displayName:'Renamed',maxOutputTokens:null,extraArgs:{temperature:0.2}})})));expect(close).toHaveBeenCalledOnce();
});
