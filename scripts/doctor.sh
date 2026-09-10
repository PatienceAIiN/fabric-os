#!/usr/bin/env bash
# doctor.sh — read-only host capability check. Prints what the OS build needs
# and what is missing. Never installs anything, never uses sudo.
set -uo pipefail
ok(){ printf '  \033[32mok\033[0m   %s\n' "$1"; }
no(){ printf '  \033[31mNO\033[0m   %s\n' "$1"; }
warn(){ printf '  \033[33mwarn\033[0m %s\n' "$1"; }

echo "== CPU / memory =="
printf '  cores: %s   ram: %s\n' "$(nproc)" "$(free -h | awk '/^Mem:/{print $2" total, "$7" avail"}')"
[[ $(free -m | awk '/^Mem:/{print $2}') -lt 9000 ]] && warn "low RAM host: always use tools/rg for builds"

echo "== virtualization =="
grep -qE 'vmx|svm' /proc/cpuinfo && ok "hardware virt (vmx/svm)" || no "no hardware virt"
[[ -e /dev/kvm ]] && { [[ -r /dev/kvm && -w /dev/kvm ]] && ok "/dev/kvm accessible" || warn "/dev/kvm exists but not accessible (kvm group?)"; } || no "/dev/kvm missing"

echo "== cgroup v2 delegation (needed by tools/rg) =="
c=$(cat /sys/fs/cgroup/user.slice/user-$(id -u).slice/cgroup.controllers 2>/dev/null)
for want in cpu memory io pids; do echo "$c" | grep -qw $want && ok "controller: $want" || no "controller missing: $want"; done

echo "== kernel security features =="
lsm=$(cat /sys/kernel/security/lsm 2>/dev/null)
for f in landlock bpf; do echo "$lsm" | grep -qw $f && ok "LSM: $f" || warn "LSM absent: $f"; done
[[ -d /sys/kernel/sched_ext ]] && ok "sched_ext present (M22 research)" || warn "sched_ext absent"

echo "== toolchain =="
for t in gcc clang make ninja meson cmake python3 git podman; do command -v $t >/dev/null && ok "$t" || no "$t missing"; done
for t in rustc cargo; do command -v $t >/dev/null && ok "$t" || warn "$t missing -> run scripts/setup-dev.sh"; done

echo "== needs sudo to install (do manually if you reach that milestone) =="
need=(); for p in qemu-system-x86_64 qemu-img bpftool flex bison protoc; do command -v $p >/dev/null || need+=("$p"); done
if [[ ${#need[@]} -eq 0 ]]; then ok "all optional system tools present"; else
  warn "missing (dnf): ${need[*]}"
  echo "     sudo dnf install qemu-kvm qemu-img edk2-ovmf bpftool flex bison protobuf-compiler kernel-devel"
fi
