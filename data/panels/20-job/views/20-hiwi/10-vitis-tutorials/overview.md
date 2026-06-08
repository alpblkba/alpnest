# Vitis Tutorials

## source

https://github.com/Xilinx/Vitis-Tutorials

## why this matters for HiWi

This repository is the broad AMD/Xilinx tutorial collection. For the HiWi meeting,
it should be used as orientation material for the Vitis/Vivado ecosystem, not as a
rabbit hole to fully master before the first task.

The useful part for our current context is the Vitis HLS flow: how a C/C++ kernel
or top function becomes RTL/IP, how simulation and synthesis are run, and how the
generated accelerator can later fit into an embedded processor system.

## current HiWi context

The HiWi work is related to HLS accelerators for embedded processors and legacy
lab rewriting. The expected first useful skill is not full Vitis mastery. The
expected first useful skill is being able to open an existing HLS project, identify
its top function and testbench, run the basic flow, read reports, and ask sane
questions about tool versions, targets, and reproducibility.

## what to focus on first

- Vitis HLS flow
- C/C++ top function to RTL/IP
- project structure
- C simulation
- C synthesis
- RTL co-simulation
- IP export
- reports and warnings
- tool version sensitivity
- embedded processor integration context

## what to avoid for now

- full Vitis ecosystem mastery
- AI Engine tutorials
- Vitis AI
- cloud acceleration flows
- advanced platform creation
- installing or debugging the full environment before supervisors clarify versions

The immediate goal is first-meeting readiness, not environment rabbit holes.

## basic HLS flow

```text
C/C++ function
-> C simulation
-> C synthesis
-> co-simulation
-> report analysis
-> IP export / Vivado integration
```

## vocabulary to refresh

- Vitis HLS
- Vivado
- C simulation
- C synthesis
- RTL co-simulation
- IP export
- HLS kernel
- top function
- testbench
- target part
- board target
- solution
- clock constraint
- latency
- initiation interval
- trip count
- LUT
- FF
- DSP
- BRAM
- pipeline warning
- AXI-lite
- AXI master
- AXI stream

## meeting-ready explanation

I reviewed the Vitis tutorial repository mainly as a map of the AMD/Xilinx tool
ecosystem. For now, I focused on the HLS-related parts: C/C++ to RTL, simulation,
synthesis, co-simulation, IP export, and how generated accelerators fit into an
embedded processor flow. I intentionally avoided trying to master the full Vitis
ecosystem before knowing the exact lab/tool version requirements.

## concrete supervisor questions

- Which Vivado/Vitis version should I use?
- Is the existing material based on Vivado HLS or Vitis HLS?
- Should we preserve legacy flow compatibility or modernize it to Vitis?
- Are we targeting pure HLS IP export, or a full Vitis platform/kernel flow?
- Which board or device is the reference target?
- Is the main first task code migration, optimization, documentation, or lab redesign?
- Should the student-facing flow be Tcl/script based or GUI based?
- What level of validation is expected: C simulation, co-simulation, or board-level validation?
