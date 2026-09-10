// SPDX-License-Identifier: GPL-2.0
// ai-native-os experimental sched_ext scheduler (M22 — RESEARCH, DO NOT LOAD
// on a daily-driver machine; it replaces the system scheduler. Test in a VM).
//
// Policy: a simple global-DSQ scheduler that gives *agent* tasks (whose PIDs
// userspace marks in the `agent_pids` map) a shorter time slice, so background
// AI-agent workloads yield to interactive/foreground tasks. This encodes the
// AI-aware deprioritization measured in scripts/sched-experiment.sh, but in the
// kernel scheduler class instead of via cgroup CPUWeight.
#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

char _license[] SEC("license") = "GPL";

/* scx kfuncs (scx_bpf_*) are declared by vmlinux.h with __weak __ksym. */

/* userspace marks background agent PIDs here (value = 1). */
struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 4096);
	__type(key, u32);
	__type(value, u8);
} agent_pids SEC(".maps");

SEC("struct_ops/ainos_select_cpu")
s32 BPF_PROG(ainos_select_cpu, struct task_struct *p, s32 prev_cpu, u64 wake_flags)
{
	bool is_idle = false;
	return scx_bpf_select_cpu_dfl(p, prev_cpu, wake_flags, &is_idle);
}

SEC("struct_ops/ainos_enqueue")
void BPF_PROG(ainos_enqueue, struct task_struct *p, u64 enq_flags)
{
	u32 pid = p->pid;
	u8 *is_agent = bpf_map_lookup_elem(&agent_pids, &pid);
	/* agents get 1/4 slice => they yield the CPU sooner (deprioritized). */
	u64 slice = is_agent ? (SCX_SLICE_DFL / 4) : SCX_SLICE_DFL;
	scx_bpf_dsq_insert(p, SCX_DSQ_GLOBAL, slice, enq_flags);
}

SEC("struct_ops/ainos_dispatch")
void BPF_PROG(ainos_dispatch, s32 cpu, struct task_struct *prev)
{
	scx_bpf_dsq_move_to_local(SCX_DSQ_GLOBAL);
}

SEC("struct_ops/ainos_init")
s32 BPF_PROG(ainos_init)
{
	return 0;
}

SEC("struct_ops/ainos_exit")
void BPF_PROG(ainos_exit, struct scx_exit_info *ei)
{
}

SEC(".struct_ops.link")
struct sched_ext_ops ainos_ops = {
	.select_cpu = (void *)ainos_select_cpu,
	.enqueue    = (void *)ainos_enqueue,
	.dispatch   = (void *)ainos_dispatch,
	.init       = (void *)ainos_init,
	.exit       = (void *)ainos_exit,
	.name       = "ainos",
};
