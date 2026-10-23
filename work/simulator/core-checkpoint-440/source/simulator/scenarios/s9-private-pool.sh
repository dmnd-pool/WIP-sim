(
set -eu
[ "$(printf '<\011>')" = '<	>' ]
printf '\nMD_SIM_SERIAL_LITERAL_TAB_VERIFIED\n'
# This runs in the original guest before its START=99 miner service.
# The archive remains read-only. Changes use the run's private SD overlay.
[ -z "$(pidof bosminer || true)" ]
[ ! -e /etc/bosminer.toml ]
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
(
set -C
cat /tmp/md-sim-pool.toml > /etc/bosminer.toml_tmp
cat /tmp/md-sim-pool.toml > /etc/bosminer.toml
)
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
    [ "$attempts" -le 120 ]
    sleep 1
done
printf '\nMD_SIM_NATIVE_LISTENERS\n'
netstat -lntp || true
for sample in $(seq 0 30); do
    printf '\nMD_SIM_PRIVATE_POOL_SAMPLE:%s\n' "$sample"
    cat /proc/uptime
    case "$sample" in
        0|5|10|12|14|16|20|25|30)
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
    [ "$sample" -eq 30 ] || sleep 1
done
printf '\nMD_SIM_PRIVATE_POOL_DIAGNOSTIC_DONE\n'
)
