(
set -eu
[ "$(sha256sum /etc/sysupgrade.conf | cut -d ' ' -f 1)" = '9e87b05fe2f12c84d1c465b396322748479f64bc40e5bab061887f0bd95020a6' ]
[ "$(cat /sys/class/net/eth0/address)" = 'd6:1b:f8:99:cd:b9' ]
[ "$(cat /tmp/mnt/boot/uEnv.txt)" = 'ethaddr=d6:1b:f8:99:cd:b9' ]
printf '\nMD_SIM_STABLE_MAC:d6:1b:f8:99:cd:b9\n'
[ "$(printf '<\011>')" = '<	>' ]
printf '\nMD_SIM_SERIAL_LITERAL_TAB_VERIFIED\n'
# This restart uses the original guest's persisted native service configuration.
# The archive remains read-only. Changes use the run's private SD overlay.
cat > /tmp/md-sim-pool.toml <<'CONFIG'
[format]
version = '1.2+'
model = 'Antminer S9'
generator = 'md-sim-private-pool-diagnostic'
timestamp = 1651224222

[[group]]
name = 'Private simulator pool'

[[group.pool]]
url = 'stratum+tcp://10.0.2.100:3333'
user = 'simulator'
password = 'x'

[hash_chain_global]
frequency = 100.0

[autotuning]
enabled = false
CONFIG
cmp /tmp/md-sim-pool.toml /etc/bosminer.toml
printf '\nMD_SIM_PRIVATE_POOL_CONFIG\n'
sha256sum /etc/bosminer.toml
cat /etc/bosminer.toml
attempts=0
until uci -q get network.lan >/dev/null; do
    attempts=$((attempts + 1))
    if [ "$((attempts % 10))" -eq 0 ]; then
        printf '\nMD_SIM_NETWORK_CONFIG_WAIT:%s\n' "$attempts"
        uci show network || true
        cat /proc/net/dev
    fi
    [ "$attempts" -le 60 ]
    sleep 1
done
uci show network
uci set network.lan.proto=static
uci set network.lan.ipaddr=10.0.2.15
uci set network.lan.netmask=255.255.255.0
uci -q delete network.lan.gateway || true
uci -q delete network.lan.dns || true
uci commit network
/etc/init.d/network restart
logread -f &
reader=$!
trap 'kill "$reader" 2>/dev/null || true' EXIT
attempts=0
while ! pidof bosminer >/dev/null; do
    attempts=$((attempts + 1))
    if [ "$attempts" -eq 1 ] || [ "$((attempts % 10))" -eq 0 ]; then
        printf '\nMD_SIM_NATIVE_STARTUP_WAIT:%s\n' "$attempts"
        cat /proc/uptime
        ps w
        cat /proc/net/dev
        cat /proc/meminfo
        ubus call service list || true
    fi
    [ "$attempts" -le 120 ]
    sleep 1
done
printf '\nMD_SIM_NATIVE_LISTENERS\n'
netstat -lntp || true
for sample in $(seq 0 120); do
    printf '\nMD_SIM_PRIVATE_POOL_SAMPLE:%s\n' "$sample"
    cat /proc/uptime
    if [ "$sample" -eq 100 ]; then
        read observed_uptime rest < /proc/uptime
        [ "${observed_uptime%%.*}" -ge 185 ]
        old_pid=$(pidof bosminer)
        case "$old_pid" in ''|*[!0-9]*) exit 1 ;; esac
        # Frame the whole record in one string. Concurrent logread output can
        # appear before the newline. The verifier requires the end delimiter.
        printf '\n%s\n' "MD_SIM_FAN_RECOVERY_RESTART_BEGIN:$observed_uptime:$old_pid:MD_SIM_FAN_RECOVERY_MARKER_END"
        /etc/init.d/bosminer restart
        attempts=0
        while :; do
            new_pid=$(pidof bosminer || true)
            case "$new_pid" in
                ''|*[!0-9]*) ;;
                *) [ "$new_pid" != "$old_pid" ] && break ;;
            esac
            attempts=$((attempts + 1))
            [ "$attempts" -le 60 ]
            sleep 1
        done
        read observed_uptime rest < /proc/uptime
        printf '\n%s\n' "MD_SIM_FAN_RECOVERY_RESTART_END:$observed_uptime:$new_pid:MD_SIM_FAN_RECOVERY_MARKER_END"
    fi
    case "$sample" in
        *[05])
            printf '\nMD_SIM_NATIVE_API_SAMPLE:%s\n' "$sample"
            # This archived BusyBox nc has no -w option. Bound the child
            # explicitly and retain failures from the real native service.
            printf '{"command":"summary+devs+notify+pools"}\n' > /tmp/md-sim-api-request
            # Keep stdin open while the native service sends its response.
            # The old immediate EOF can make this BusyBox nc exit too soon.
            (cat /tmp/md-sim-api-request; sleep 2) | nc 127.0.0.1 4028 > /tmp/md-sim-api-response 2>&1 &
            api_client=$!
            sleep 1
            kill "$api_client" 2>/dev/null || true
            wait "$api_client" 2>/dev/null || true
            tr '\000' '\n' < /tmp/md-sim-api-response
            printf '\nMD_SIM_NATIVE_SERVICE_STATE:%s\n' "$sample"
            ubus call service list '{"name":"bosminer"}' || true
            ;;
    esac
    [ "$sample" -eq 120 ] || sleep 1
done
printf '\nMD_SIM_PRIVATE_POOL_DIAGNOSTIC_DONE\n'
)
