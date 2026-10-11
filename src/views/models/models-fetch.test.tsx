import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,render,screen,waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {WorkspaceProvider} from '../../lib/workspace';
import {request} from '../../lib/api';
import {FetchModels} from './FetchModels';
vi.mock('../../lib/api',()=>({request:vi.fn(),errorText:(e:unknown)=>String(e)}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
test('models-fetch selects new IDs only and requires explicit confirmation',async()=>{
 const mock=vi.mocked(request);const workspace={revision:0,targetPath:'/test',catalog:{providers:[{id:'p',name:'Provider'}],models:[{providerId:'p',apiModelId:'existing'}]}};
 mock.mockImplementation(async(command)=>command==='fetch_models'?{ids:['existing','new/one','new/two'],elapsedMs:12,incomplete:true}:workspace);
 const user=userEvent.setup();function Wrapper(){return <WorkspaceProvider><FetchModels onClose={()=>{}}/></WorkspaceProvider>;}render(<Wrapper/>);
 await screen.findByText('Provider');
 // Select after the asynchronous workspace load, as users do when opening the dialog.
 await user.selectOptions(screen.getByLabelText('供应商'),'p');await user.click(screen.getByRole('button',{name:'获取列表'}));await screen.findByText('new/one');expect(mock.mock.calls.some(([c])=>c==='import_models')).toBe(false);
 await user.click(screen.getByRole('button',{name:'选择当前搜索结果'}));await user.click(screen.getByRole('button',{name:'添加所选 2 项'}));
 await waitFor(()=>expect(mock).toHaveBeenCalledWith('import_models',{providerId:'p',ids:['new/one','new/two'],revision:0}));
});
