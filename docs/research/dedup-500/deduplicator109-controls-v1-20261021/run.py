"""Run unchanged Lightroom entry source with authored Lua service adapters only."""
from pathlib import Path
import hashlib
from lupa import LuaRuntime
R=Path(__file__).resolve().parents[4]
p=R/'docs/research/dedup-500/evidence/teran--deduplicator/02-FindDuplicates.lua'
b=p.read_bytes()
assert hashlib.sha256(b).hexdigest()=='21f191fe5772867eb50fadd4a8973b325448cbcc762cd8d9d8265f5fc0f61c33'
setup=r'''
searches=0; indexed=0; done=0; tasks={}; canceled=false; available=true; exit_status=0
plugin_name='synthetic';plugin_version='synthetic';log_target='none';WIN_ENV=false
_PLUGIN={path='/synthetic/plugin'}
local noop=function()end
local log=setmetatable({}, {__index=function()return noop end})
local collection={localIdentifier=1,addPhotos=noop}
local photo={getRawMetadata=function()return '/synthetic/image.jpg'end,
getFormattedMetadata=function()return 'image.jpg'end,
checkPhotoAvailability=function()return available end}
local cat={withWriteAccessDo=function(self,name,fn)fn()end,
createCollection=function()return collection end,
getMultipleSelectedOrAllPhotos=function()return {photo,photo}end}
local progress={setCancelable=noop,isCanceled=function()return canceled end,
setPortionComplete=noop,setCaption=noop,done=function()done=done+1 end}
local services={LrApplication={activeCatalog=function()return cat end},
LrDialogs={message=noop,showBezel=noop},LrFileUtils={moveToTrash=noop},
LrLogger=function()return log end,
LrPathUtils={standardizePath=function(p)return p end,
getStandardFilePath=function()return '/synthetic/tmp'end,child=function(a,b)return a..'/'..b end},
LrProgressScope=function()return progress end,
LrSystemInfo={summaryString=function()return 'adapter'end,is64Bit=function()return true end},
LrTasks={startAsyncTask=function(fn)table.insert(tasks,fn)end,
execute=function()indexed=indexed+1;return exit_status end}}
import=function(n)return assert(services[n],n)end
package.preload.JSON=function()return {decode=function()error('Unexpected JSON decode')end}end
package.preload.Info=function()return {}end
io.popen=function()searches=searches+1;return {read=function()return ''end,close=noop}end
'''
for name,cancel,avail,status,want in [
 ('cancellation still searches partial database',True,True,0,0),
 ('index subprocess failures still proceed to search',False,True,1,2),
 ('unavailable photos are skipped but search still runs',False,False,0,0),
 ('normal positive control indexes both photos then searches',False,True,0,2)]:
 lua=LuaRuntime();lua.execute(setup);lua.execute(b.decode());g=lua.globals()
 g.canceled=cancel;g.available=avail;g.exit_status=status
 lua.execute('tasks[1](); tasks[2]()')
 assert g.indexed==want and g.searches==1 and g.done==1,(name,g.indexed,g.searches,g.done)
 print('PASS',name,': indexed=',g.indexed,'searches=',g.searches)
print('4 controls passed; no Lightroom SDK, imgsum binary, real catalog, shell, or image decoding executed')
