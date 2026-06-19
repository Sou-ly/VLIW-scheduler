<div align="center">

# VLIW-470 Instruction Scheduler

**A compiler back-end that turns scalar assembly into wide, cycle-accurate VLIW schedules —
with software pipelining, rotating registers, and predication.**

[![Rust](https://img.shields.io/badge/Rust-2021-CE422B?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![reference cases](https://img.shields.io/badge/reference%20cases-17%2F17%20passing-3FB950)](#testing)
[![unit tests](https://img.shields.io/badge/unit%20tests-14%20passing-3FB950)](#testing)
[![runtime deps](https://img.shields.io/badge/runtime%20deps-0-1F6FEB)](Cargo.toml)

</div>

---

```
   scalar in                                              wide schedule out
  ───────────                                            ──────────────────
  addi x2,x1,1                ┌────────┬────────┬────────┬────────┬────────┐
  ld   x5,0(x2)     ╮         │  ALU0  │  ALU1  │  Mult  │  Mem   │ Branch │
  mulu x6,x5,x4     │  ──▶    ├────────┼────────┼────────┼────────┼────────┤
  mulu x3,x3,x5     │ pack    │  addi  │  mov   │  mulu  │   ld   │  loop  │ ◀─ 1 cycle,
  st   x6,0(x2)     ╯         └────────┴────────┴────────┴────────┴────────┘    5 ops
  ...                          independent ops issue together, hazards respected
```

> On VLIW hardware there is **no out-of-order engine and no rename unit at runtime** — the
> compiler alone decides what issues when. Get the schedule wrong and the program is either
> slow or incorrect. This project is a compact, end-to-end implementation of that back-end.

---

## What it does

Given a stream of scalar assembly, the scheduler emits a cycle-by-cycle schedule of **bundles**
(up to 5 operations issued per cycle), honoring every data hazard and structural limit of the
machine. It supports the two strategies a real VLIW back-end needs:

| Mode | Flag | Strategy |
|------|------|----------|
| **`loop`** | `--no_pip` | List-style ASAP scheduling + acyclic register renaming |
| **`loop.pip`** | `--pip` | **Software pipelining** via modulo scheduling — rotating registers and predication overlap successive iterations |

---

## The VLIW-470 machine

One bundle issues per cycle, one op per functional unit:

| Slot | Unit | Count | Latency | Handles |
|:----:|------|:-----:|:-------:|---------|
| 0, 1 | **ALU** | 2 | 1 cycle | `add` `addi` `sub` `mov` |
| 2 | **Mult** | 1 | 3 cycles | `mulu` |
| 3 | **Mem** | 1 | 1 cycle | `ld` `st` |
| 4 | **Branch** | 1 | 1 cycle | `loop` `loop.pip` |

Plus the machinery software pipelining relies on: a large register file with **rotating
registers**, **predicate registers** (`p32`+), and the special registers `LC` (loop count) and
`EC` (epilogue count).

<details>
<summary><b>Instruction set</b></summary>

```asm
add   xD, xA, xB         ; xD = xA + xB
addi  xD, xA, imm        ; xD = xA + imm
sub   xD, xA, xB
mulu  xD, xA, xB         ; 3-cycle latency
ld    xD, imm(xA)        ; load
st    xS, imm(xA)        ; store
mov   xD, imm | xD, xS | LC, imm | EC, imm | pN, bool
loop      target         ; backward branch (non-pipelined)
loop.pip  target         ; backward branch (software-pipelined)
```
</details>

---

## How it works

A small but complete compiler pipeline:

```
  input.json                                                 schedule.json
      │                                                            ▲
      ▼                                                            │
  ┌────────┐   ┌────────────┐   ┌───────────┐   ┌──────────────┐  │
  │ parse  │──▶│ dependency │──▶│  schedule │──▶│   register   │──┘
  │        │   │  analysis  │   │ (bundles) │   │  allocation  │
  └────────┘   └────────────┘   └───────────┘   └──────────────┘
   parser.rs    dependency.rs    scheduler.rs     scheduler.rs /
                                 pip_scheduler.rs  pip_reg_alloc.rs
```

1. **Parse** — text assembly into a typed `Instruction` enum.
2. **Dependency analysis** — for every operand, find its producer and classify the edge. Loops
   make this the interesting part, so edges are split into four kinds:

   | Kind | Meaning |
   |------|---------|
   | **Local** | producer and consumer in the same straight-line region |
   | **Inter-loop** | value produced in one iteration, read in the next |
   | **Loop-invariant** | produced before the loop, read inside it |
   | **Post-loop** | produced inside the loop, read after it exits |

3. **Scheduling** — place each op at a (cycle, unit) with no latency or structural violation.
   The pipelined path runs **modulo scheduling**: search for the smallest *Initiation Interval*
   (II) at which a new iteration can launch, then fold the loop body into that II-cycle steady state.
4. **Register allocation** — rename destinations to kill false dependencies; the pipelined path
   additionally assigns **rotating registers** across overlapping iterations and inserts
   **predicates** so the prologue and epilogue fill and drain the pipeline correctly.

<details>
<summary><b>Module map</b></summary>

| File | Responsibility |
|------|----------------|
| `src/vliw470.rs` | Entry point: I/O and pipeline wiring |
| `src/instruction.rs` | Instruction / bundle / functional-unit model |
| `src/parser.rs` | Assembly into `Instruction` |
| `src/dependency.rs` | Dependency graph construction and classification |
| `src/scheduler.rs` | Non-pipelined scheduling + register renaming |
| `src/pip_scheduler.rs` | Modulo scheduler (finds II, lays out the loop) |
| `src/pip_reg_alloc.rs` | Rotating-register allocation and predication |
</details>

---

## Quick start

```bash
./build.sh          # cargo build --release

# Schedule one program two ways:  ./run.sh <input> <loop_out> <pip_out>
./run.sh tests/17/input.json out.simple.json out.pip.json
```

Or drive the binary directly:

```bash
cargo run --release -- <input.json> <output.json> --no_pip   # loop schedule
cargo run --release -- <input.json> <output.json> --pip      # software-pipelined
```

---

## See it work

Input is a JSON array of assembly lines; output is a JSON array of bundles, each a 5-element
array `[ALU0, ALU1, Mult, Mem, Branch]`.

**A loop that loads, accumulates with a loop-invariant, and stores:**

```json
["mov LC, 100", "mov x2, 10", "mov x3, 0x1000",
 "ld x4, 0(x3)", "add x4, x4, x2", "st x4, 0(x3)", "loop 3"]
```

**`--no_pip` output** — registers renamed, the load hoisted to hide its latency:

```text
┌──────────────────┬─────────────┬──────┬───────────────┬──────────┐
│ ALU0             │ ALU1        │ Mult │ Mem           │ Branch   │
├──────────────────┼─────────────┼──────┼───────────────┼──────────┤
│ mov LC, 100      │ mov x1, 10  │ nop  │ nop           │ nop      │
│ mov x2, 4096     │ nop         │ nop  │ nop           │ nop      │
│ nop              │ nop         │ nop  │ ld  x3, 0(x2) │ nop      │
│ add x4, x3, x1   │ nop         │ nop  │ nop           │ nop      │
│ nop              │ nop         │ nop  │ st  x4, 0(x2) │ loop 2   │
└──────────────────┴─────────────┴──────┴───────────────┴──────────┘
```

In the **`--pip`** schedule, loop-body ops carry rotating registers and predicates
(`(p32) ...`) so consecutive iterations overlap in the steady state — that is the software pipeline.

---

## Testing

`tests/` holds 17 cases, each a folder with an `input.json`, a one-line `desc.txt`, and reference
outputs. They climb the full feature ladder: slot mapping, single- and multi-cycle hazards,
dependency chains, and every loop dependency class.

```bash
./runall.sh     # generate simple.json + pip.json for every test
./testall.sh    # diff each against its reference, print pass/fail
cargo test      # unit tests over the analysis and scheduling internals
```

> **Status:** 17/17 reference cases pass for **both** schedule modes; all 14 unit tests pass.

A small simulator and HTML visualizer in `simulator/` let you step through a schedule cycle by
cycle — see `simulator/Readme.md`.

---

## Reproducible environment

A `Dockerfile` pins a Rust + Python toolchain so the build and tests run identically anywhere:

```bash
docker build -t vliw470 .
docker run -it -v "$(pwd)":/work vliw470
# inside:  ./runall.sh && ./testall.sh
```

---

<div align="center">
<sub>Built as a two-person systems project exploring VLIW compiler back-ends — modulo scheduling,
rotating registers, and predication — then cleaned up and documented.</sub>
</div>
