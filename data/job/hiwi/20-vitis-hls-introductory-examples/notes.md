# Vitis HLS Introductory Examples Notes

## high-value examples

Focus on these categories first:

- Pipelining
- Array
- Interface
- Modeling
- Task_level_Parallelism

The important thing is not to memorize the examples. The important thing is to
understand what each pragma changes in generated hardware and how that shows up
in synthesis reports.

## minimal practical competency

I should be able to say:

```text
I reviewed the basic Vitis HLS flow and looked at pipelining, array partitioning,
AXI interfaces, and dataflow-style task parallelism examples.
```

## report reading checklist

When reading a synthesis report, look for:

- clock period
- latency
- initiation interval
- loop summary
- resource utilization
- warnings about dependencies
- warnings about memory ports
- whether requested II was achieved

## likely debug questions

- Is II limited by loop-carried dependency?
- Is II limited by memory port conflicts?
- Would array partitioning help?
- Is the interface correct for the intended system?
- Does co-simulation pass?
- Is the script using the right tool version and target part?
