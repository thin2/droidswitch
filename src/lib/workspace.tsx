import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { request, errorText } from './api';
import type { Workspace } from './types';
interface Context { workspace:Workspace|null; busy:boolean; error:string; notice:string; clearError:()=>void; reload:()=>Promise<void>; mutate:(command:string,args?:Record<string,unknown>)=>Promise<boolean>; setNotice:(text:string)=>void }
const WorkspaceContext=createContext<Context|null>(null);
export function WorkspaceProvider({children}:{children:ReactNode}) {
  const [workspace,setWorkspace]=useState<Workspace|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState('');
  async function reload(){setBusy(true);try{setWorkspace(await request<Workspace>('load_workspace'));setError('');}catch(e){setError(errorText(e));}finally{setBusy(false);}}
  useEffect(()=>{void reload();},[]);
  async function mutate(command:string,args:Record<string,unknown>={}){
    if(!workspace || busy)return false;setBusy(true);setError('');setNotice('');
    try{setWorkspace(await request<Workspace>(command,{...args,revision:workspace.revision}));setNotice('草稿已保存');return true;}catch(e){setError(errorText(e));return false;}finally{setBusy(false);}
  }
  return <WorkspaceContext.Provider value={{workspace,busy,error,notice,clearError:()=>setError(''),reload,mutate,setNotice}}>{children}</WorkspaceContext.Provider>;
}
export function useWorkspace(){const context=useContext(WorkspaceContext);if(!context)throw new Error('Workspace provider missing');return context;}
