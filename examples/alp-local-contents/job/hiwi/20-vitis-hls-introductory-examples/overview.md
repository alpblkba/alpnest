# Vitis HLS Introductory Examples

## source

https://github.com/Xilinx/Vitis-HLS-Introductory-Examples

## why this matters for HiWi

This is the more directly useful repository for immediate HiWi preparation. It
contains small focused HLS examples with source code, testbenches, and scripted
flows. This is closer to the kind of work expected from an HLS accelerator lab or
legacy lab rewrite.

The goal is to be able to receive an existing HLS task, run it, understand the
main pragmas, inspect reports, and document what is happening.

## priority areas

### 1. pipelining

Pipelining is the first optimization concept to understand.

Key idea:

- latency tells how long one operation or loop takes
- initiation interval tells how often a new operation can start
- II=1 is often the desired throughput target
- dependencies and memory port limits can prevent II=1

Typical pragma:

```cpp
for (...) {
    #pragma HLS PIPELINE II=1
    // loop body
}
```

Main question:

```text
Can this loop accept/start new work every cycle? If not, why not?
```

Things to inspect:

- `#pragma HLS PIPELINE`
- achieved II vs requested II
- loop-carried dependencies
- memory access conflicts
- synthesis report warnings

### 2. array partitioning

Arrays often become memories in FPGA synthesis. Memories have limited ports, so a
loop may fail to achieve good II because it cannot read/write enough elements in
the same cycle.

Typical pragma:

```cpp
#pragma HLS ARRAY_PARTITION variable=A complete dim=1
```

Key idea:

- complete partition can turn array elements into independent registers
- block partition splits the array into contiguous blocks
- cyclic partition distributes elements in an interleaved way
- more memory parallelism can improve throughput
- more partitioning can increase resource usage

Things to inspect:

- `#pragma HLS ARRAY_PARTITION`
- complete partition
- block partition
- cyclic partition
- BRAM/register tradeoffs
- whether partitioning helps pipelining reach lower II

### 3. interfaces

Interfaces explain how the HLS top function talks to the rest of the system.
This is critical for embedded processor + accelerator work.

Common interface types:

```text
s_axilite -> control/status register interface
m_axi     -> memory-mapped global memory / DDR access
axis      -> streaming input/output
```

Typical shape:

```cpp
void top(int *in, int *out, int size) {
    #pragma HLS INTERFACE m_axi port=in
    #pragma HLS INTERFACE m_axi port=out
    #pragma HLS INTERFACE s_axilite port=size
    #pragma HLS INTERFACE s_axilite port=return

    // accelerator body
}
```

Things to inspect:

- top function arguments
- return port
- AXI-lite control
- AXI master memory ports
- AXI stream ports
- how the interface changes generated hardware

### 4. task-level parallelism / dataflow

This is useful after the basic pipelining and interface ideas are clear.

Key idea:

- split computation into producer/consumer stages
- connect stages using streams/FIFOs
- allow stages to overlap in time

Typical concepts:

- `#pragma HLS DATAFLOW`
- `hls::stream`
- producer/consumer functions
- FIFO sizing
- deadlock or co-simulation issues

## script flow

Look for Tcl or Python scripts rather than relying only on the GUI.

Example Tcl-style command:

```bash
vitis-run --mode hls --tcl run_hls.tcl
```

Example Python-style command:

```bash
vitis -s run.py
```

Important things inside scripts:

- create/open project
- set top function
- add source files
- add testbench files
- set target part or board
- set clock period
- run C simulation
- run C synthesis
- run co-simulation
- export design/IP

## what to be able to explain by the meeting

- what the HLS top function is
- why a C/C++ testbench exists
- what C simulation verifies
- what C synthesis generates
- what co-simulation checks
- what latency means
- what initiation interval means
- why array partitioning affects performance
- how AXI interfaces connect the accelerator to memory/processor/streams
- why exact tool versions matter

## likely first-task workflow

If I receive an existing HLS lab/project, the first steps should be:

1. identify the top function
2. identify source and testbench files
3. find the build script or project file
4. check the target device/board and clock
5. run C simulation
6. run C synthesis
7. inspect reports for latency, II, and resources
8. run co-simulation if available
9. document issues, version assumptions, and reproducibility gaps

## meeting-ready explanation

I focused more on the introductory HLS examples because they are close to the type
of work I expect in the HiWi task. I looked at pipelining, array partitioning,
interfaces, and scripted HLS flows. My current goal is not full tool mastery, but
being able to run an existing HLS project, read the reports, and ask useful
questions about lab structure, tool versions, and validation expectations.
