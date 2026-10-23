set -e
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
mknod /dev/mmcblk0 b 179 0
mkdir /tmp/persistent
mount -t ext2 /dev/mmcblk0 /tmp/persistent
if [ ! -d /tmp/persistent/etc ]; then cp -a /etc /tmp/persistent/etc; sync; fi
mount --bind /tmp/persistent/etc /etc
ifconfig lo 127.0.0.1 up
ifconfig eth0 10.0.2.15 netmask 255.255.255.0 up
/sbin/ubusd >/tmp/ubusd.log 2>&1 &
i=0; while [ ! -S /var/run/ubus.sock ] && [ "$i" -lt 10 ]; do sleep 1; i=$((i+1)); done
/sbin/rpcd >/tmp/rpcd.log 2>&1 &
i=0; while ! ubus -t 1 list session && [ "$i" -lt 10 ]; do sleep 1; i=$((i+1)); done
/sbin/procd -s /var/run/ubus.sock >/tmp/procd.log 2>&1 &
i=0; while ! ubus -t 1 call system board && [ "$i" -lt 10 ]; do sleep 1; i=$((i+1)); done
/usr/sbin/uhttpd -f -h /www -p 0.0.0.0:8000 -x /cgi-bin -u /ubus >/tmp/uhttpd.log 2>&1 &
sleep 1
if lua - <<'LUA'
local h=require('socket.http')
local j=require('cjson')
-- This is a diagnostic client deadline for nested CPU emulation.
h.TIMEOUT=30
local chunks={}
local size=0
local function sink(chunk)
 if chunk then
  size=size+#chunk
  if size>1048576 then return nil,'login response exceeded 1 MiB' end
  chunks[#chunks+1]=chunk
 end
 return 1
end
local ok,code,headers,status=h.request{url='http://127.0.0.1:8000/cgi-bin/luci/',method='GET',sink=sink,redirect=false}
local body=table.concat(chunks)
print('MD_SIM_NATIVE_LOGIN_PROBE_RESULT',ok,code,status,#body,j.encode(headers or {}))
print('MD_SIM_NATIVE_LOGIN_PROBE_BODY',body:sub(1,4096))
assert(ok==1 and code==403 and body:find('luci_password',1,true),'native login page did not start')
print('MD_SIM_NATIVE_LOGIN_PROBE_STATUS',code)
LUA
then :
else
 for log in /tmp/uhttpd.log /tmp/procd.log /tmp/rpcd.log /tmp/ubusd.log; do
  printf '\nMD_SIM_NATIVE_SERVICE_LOG %s\n' "$log"
  dd if="$log" bs=4096 count=1 2>/dev/null
 done
 printf '\nMD_SIM_NATIVE_PROCESS_STATE\n'
 ps w | dd bs=4096 count=2 2>/dev/null
 for pid in $(pidof uhttpd); do
  printf '\nMD_SIM_NATIVE_UHTTPD_WAIT %s\n' "$pid"
  cat "/proc/$pid/wchan"
 done
 exit 1
fi
dmesg
printf '\nMD_SIM_NATIVE_NETWORK_READY\n'
