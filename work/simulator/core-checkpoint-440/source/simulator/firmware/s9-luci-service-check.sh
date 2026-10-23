mount -t proc proc /proc
dmesg -n 1
mount -t sysfs sysfs /sys
mount -t tmpfs tmpfs /tmp
mount -t tmpfs tmpfs /dev
mknod /dev/null c 1 3
mknod /dev/zero c 1 5
mknod /dev/random c 1 8
mknod /dev/urandom c 1 9
mknod /dev/console c 5 1
chmod 666 /dev/null /dev/zero /dev/random /dev/urandom
mkdir -p /tmp/run /tmp/lock
cp -a /etc /tmp/etc
mount --bind /tmp/etc /etc
ifconfig lo 127.0.0.1 up
/sbin/ubusd > /tmp/ubusd.log 2>&1 &
i=0; while [ ! -S /var/run/ubus.sock ] && [ "$i" -lt 10 ]; do sleep 1; i=$((i+1)); done
/sbin/rpcd > /tmp/rpcd.log 2>&1 &
i=0; while ! ubus -t 1 list session && [ "$i" -lt 10 ]; do sleep 1; i=$((i+1)); done
/sbin/procd -s /var/run/ubus.sock > /tmp/procd.log 2>&1 &
i=0; while ! ubus -t 1 call system board && [ "$i" -lt 10 ]; do sleep 1; i=$((i+1)); done
/usr/sbin/uhttpd -f -h /www -p 127.0.0.1:8000 -x /cgi-bin -u /ubus > /tmp/uhttpd.log 2>&1 &
sleep 1
lua - <<'LUA'
local h=require('socket.http')
local l=require('ltn12')
local j=require('cjson')
h.TIMEOUT=5
local base='http://127.0.0.1:8000'
local cookie=nil
local function req(path,body,ctype)
 local out={}
 local headers={}
 if cookie then headers.cookie=cookie end
 if body then headers['content-type']=ctype or 'application/json'; headers['content-length']=tostring(#body) end
 local ok,code,head,status=h.request{url=base..path,method=body and 'POST' or 'GET',source=body and l.source.string(body),sink=l.sink.table(out),headers=headers,redirect=false}
 local text=table.concat(out)
 print('NATIVE_HTTP',path,ok,code,status,j.encode(head or {}),#text)
 assert(ok==1,'request failed: '..tostring(code))
 return code,head,text
end
local c,heads,body=req('/cgi-bin/luci/')
assert(c==403 and body:find('luci_password',1,true),'login form missing')
c,heads,body=req('/cgi-bin/luci/','luci_username=root&luci_password=','application/x-www-form-urlencoded')
print('LOGIN_BODY',body)
if heads['set-cookie'] then cookie=heads['set-cookie']:match('([^;]+)'); print('COOKIE_PRESENT',true) end
assert(cookie,'login did not create cookie')
c,heads,body=req('/cgi-bin/luci/admin/status/overview')
print('OVERVIEW_BODY',body)
assert(c==200,'authenticated overview failed')
c,heads,body=req('/luci-static/bootstrap/cascade.css')
assert(c==200 and #body>100,'native asset missing')
local function hex(s) return (s:gsub('.',function(c) return string.format('%02x',string.byte(c)) end)) end
print('NATIVE_ASSET_HEX',hex(body))
c,heads,body=req('/cgi-bin/luci/rpc/auth',j.encode{id=1,method='login',params={'root',''}})
print('RPC_AUTH',body)
assert(c==200,'RPC authentication failed')
local sid=j.decode(body).result
assert(type(sid)=='string','RPC session missing')
c,heads,body=req('/cgi-bin/luci/rpc/sys?auth='..sid,j.encode{id=2,method='uptime',params={}})
print('UPTIME',body)
assert(c==200 and type(j.decode(body).result)=='number','live uptime unavailable')
local seq=2
local function rpc(service,method,params)
 seq=seq+1
 local code,headers,text=req('/cgi-bin/luci/rpc/'..service..'?auth='..sid,j.encode{id=seq,method=method,params=params})
 print('NATIVE_RPC',service,method,text)
 assert(code==200,'RPC HTTP failure')
 local value=j.decode(text)
 assert(value.id==seq and (value.error==nil or value.error==j.null),'RPC error')
 return value.result
end
local saved=rpc('fs','readfile',{'/etc/config/uhttpd'})
assert(type(saved)=='string' and #saved>100,'original configuration bytes missing')
local config=rpc('uci','get_all',{'uhttpd','main'})
assert(type(config)=='table' and config.max_connections=='100','original configuration missing')
assert(rpc('uci','set',{'uhttpd','main','max_connections','101'})==true,'configuration write failed')
assert(rpc('uci','commit',{'uhttpd'})==true,'configuration commit failed')
assert(rpc('uci','get',{'uhttpd','main','max_connections'})=='101','configuration readback failed')
local function pid() local f=assert(io.popen('pidof uhttpd')); local s=f:read('*a'):match('%d+'); f:close(); return assert(s,'service PID missing') end
local before=pid()
os.execute('killall uhttpd')
os.execute('/usr/sbin/uhttpd -f -h /www -p 127.0.0.1:8000 -x /cgi-bin -u /ubus >/tmp/uhttpd-restart.log 2>&1 &')
require('socket').sleep(1)
local after=pid(); assert(after~=before,'service PID did not change'); print('NATIVE_SERVICE_RESTART',before,after)
assert(rpc('uci','get',{'uhttpd','main','max_connections'})=='101','service restart lost committed configuration')
assert(rpc('fs','writefile',{'/etc/config/uhttpd',saved})==1,'configuration restore failed')
assert(rpc('fs','readfile',{'/etc/config/uhttpd'})==saved,'restored configuration bytes differ')
assert(rpc('uci','get',{'uhttpd','main','max_connections'})=='100','restored configuration value differs')
print('NATIVE_LUCI_LOGIN_ASSET_LIVE_CONFIG_RESTORE_SERVICE_RESTART_COMPLETE')
LUA
native_status=$?
cat /tmp/uhttpd.log /tmp/procd.log /tmp/rpcd.log
dmesg
test "$native_status" = 0
