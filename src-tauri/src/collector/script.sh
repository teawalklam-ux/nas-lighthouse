# nas-lighthouse collector. Delivered over SSH stdin. Writes nothing on the NAS.
#
# Emits one "=BEGIN … =END" frame per interval on stdout. Every line is
# "=KEY value…" so the desktop app can parse it without a shell on its side.
#
# The block between the config markers is replaced by the desktop app before
# the script is piped over SSH. The defaults below let it run standalone:
#   ssh my-nas bash -s < script.sh
set -u

# === nas-lighthouse:config:begin ===
INTERVAL=2
ENABLE_NET_OUT=1
IFACE=''
MOUNTS=()
PROBE_HOSTS='1.1.1.1 8.8.8.8'
LINK_AGENT=''
# === nas-lighthouse:config:end ===

TICK=0

# Interface carrying the default route. Falls back to the busiest non-virtual
# interface in /proc/net/dev so bonded, bridged and plain setups all report.
detect_iface() {
  iface=$(ip route show default 2>/dev/null | awk '{ for (i = 1; i < NF; i++) if ($i == "dev") { print $(i + 1); exit } }')
  if [ -n "${iface:-}" ]; then
    echo "$iface"
    return
  fi
  awk -F'[ :]+' '
    NR > 2 {
      name = $2
      if (name == "lo") next
      if (name ~ /^(veth|docker|br-|virbr|tap|tun|dummy|kube|cni|flannel|zt|tailscale)/) next
      if ($3 + 0 > best) { best = $3 + 0; pick = name }
    }
    END { if (pick != "") print pick }
  ' /proc/net/dev 2>/dev/null
}

# Real, user-visible filesystems only: block-backed, not a system or container
# mount, one line per device.
emit_disks() {
  if [ "${#MOUNTS[@]}" -gt 0 ]; then
    for mount in "${MOUNTS[@]}"; do
      df -B1 -P -- "$mount" 2>/dev/null | awk -v mount="$mount" 'NR > 1 { print "=DISK", $2, $3, mount }'
    done
    return
  fi
  df -B1 -P 2>/dev/null | awk '
    NR > 1 {
      dev = $1; mount = $6
      if (dev !~ /^\// && dev !~ /^[^/]+\//) next
      if (dev ~ /^\/dev\/(loop|ram)/) next
      if (mount ~ /^\/(dev|proc|sys|run|boot|snap|rootfs)($|\/)/) next
      if (mount ~ /\/(docker|containers|kubelet)($|\/)/) next
      if (seen[dev]++) next
      print "=DISK", $2, $3, mount
    }'
}

# TCP reachability with latency. nc when available, bash /dev/tcp otherwise.
probe_tcp() {
  host="$1"
  start=$(date +%s%N 2>/dev/null || echo 0)
  if command -v nc >/dev/null 2>&1; then
    nc -z -w 2 "$host" 443 >/dev/null 2>&1 || return 1
  else
    if command -v timeout >/dev/null 2>&1; then
      timeout 3 bash -c 'exec 3<>"/dev/tcp/$1/443"' _ "$host" >/dev/null 2>&1 || return 1
    else
      return 2
    fi
  fi
  end=$(date +%s%N 2>/dev/null || echo 0)
  if [ "$start" != 0 ] && [ "$end" != 0 ]; then
    echo $(( (end - start) / 1000000 ))
  else
    echo 0
  fi
  return 0
}

if [ -z "$IFACE" ]; then
  IFACE=$(detect_iface)
fi
[ -n "${IFACE:-}" ] || IFACE=lo

while true; do
  ts=$(date +%s%N 2>/dev/null || date +%s)
  echo "=BEGIN $ts"

  cpu=$(awk '/^cpu / { print $2,$3,$4,$5,$6,$7,$8,$9,$10,$11; exit }' /proc/stat)
  echo "=CPU $cpu"

  mem=$(awk '
    /^MemTotal:/ { t=$2 }
    /^MemAvailable:/ { a=$2 }
    /^SwapTotal:/ { st=$2 }
    /^SwapFree:/ { sf=$2 }
    END { print t+0, a+0, st+0, sf+0 }
  ' /proc/meminfo)
  echo "=MEM $mem"

  echo "=LOAD $(awk '{ print $1,$2,$3 }' /proc/loadavg)"
  echo "=CORES $(nproc 2>/dev/null || echo 0)"
  echo "=UPTIME $(awk '{ print $1 }' /proc/uptime)"

  net=$(awk -v iface="$IFACE" '
    $1 == iface":" { print $2, $10; exit }
  ' /proc/net/dev)
  echo "=NET $IFACE $net"

  emit_disks

  tr '\n' '|' < /proc/mdstat 2>/dev/null | awk '{ print "=MDSTAT " $0 }'

  for d in /sys/class/hwmon/hwmon*; do
    [ -d "$d" ] || continue
    name=$(cat "$d/name" 2>/dev/null) || continue
    [ -n "$name" ] || continue
    for t in "$d"/temp*_input; do
      [ -f "$t" ] || continue
      val=$(cat "$t" 2>/dev/null) || continue
      echo "=TEMP $name ${t##*/} $val"
    done
  done

  if [ $((TICK % 3)) -eq 0 ]; then
    docker ps --format '{{json .}}' 2>/dev/null | while IFS= read -r line; do
      [ -n "$line" ] && echo "=DOCKER $line"
    done
  fi

  if [ $((TICK % 15)) -eq 0 ]; then
    if [ "${ENABLE_NET_OUT}" = "1" ] && [ -n "$PROBE_HOSTS" ] && { command -v nc >/dev/null 2>&1 || command -v timeout >/dev/null 2>&1; }; then
      out=down
      for host in $PROBE_HOSTS; do
        if ms=$(probe_tcp "$host"); then
          out="ok $ms"
          break
        fi
      done
      echo "=NET_OUT $out"
    fi
    # Vendor remote-access agent, e.g. UGREEN Link. Opt-in: without a process
    # name the app reports "unknown" rather than guessing.
    if [ -n "$LINK_AGENT" ]; then
      if pgrep -x "$LINK_AGENT" >/dev/null 2>&1; then
        echo "=LINK_AGENT 1"
      else
        echo "=LINK_AGENT 0"
      fi
    fi
  fi

  echo "=END"
  TICK=$((TICK + 1))
  sleep "$INTERVAL"
done
