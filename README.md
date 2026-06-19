# VLIW-470 Instruction Scheduler

A static instruction scheduler and register allocator for **VLIW-470**, a
synthetic Very Long Instruction Word (VLIW) machine. Given a stream of scalar
assembly, it packs independent operations into wide instruction *bundles* that
the machine issues in a single cycle — turning sequential code into a
cycle-by-cycle schedule that respects every data hazard and structural
constraint of the pipeline.

It implements the two scheduling strategies a real VLIW compiler back-end needs:

- **`loop`** — list-style scheduling with acyclic register renaming.
- **`loop.pip`** — **software pipelining** via modulo scheduling, with rotating
  registers and predication to overlap loop iterations.

Written in **Rust**, with no external runtime dependencies.

---

## Why this is interesting

VLIW machines push the hard scheduling problem out of the hardware and into the
compiler: there is no out-of-order engine and no register-rename unit at
runtime, so *correctness and performance both depend entirely on the schedule
the compiler emits*. This project is a compact, end-to-end implementation of
that back-end — dependency analysis, scheduling, register allocation, and
software pipelining — small enough to read in an afternoon.

---

## The VLIW-470 machine

Each cycle the machine issues one **bundle** of up to **5 operations**, one per
functional unit:

| Slot | Unit    | Count | Latency  | Handles                         |
|------|---------|-------|----------|---------------------------------|
| 0,1  | ALU     | 2     | 1 cycle  | `add` `addi` `sub` `mov`        |
| 2    | Mult    | 1     | 3 cycles | `mulu`                          |
| 3    | Mem     | 1     | 1 cycle  | `ld` `st`                       |
| 4    | Branch  | 1     | 1 cycle  | `loop` `loop.pip`               |

It also exposes the machinery needed for software pipelining: a large register
file, **rotating registers**, **predicate registers** (`p32`+), and two special
registers — `LC` (loop count) and `EC` (epilogue count).

### Instruction set

```
add   xD, xA, xB         # xD = xA + xB
addi  xD, xA, imm        # xD = xA + imm
sub   xD, xA, xB
mulu  xD, xA, xB         # 3-cycle latency
ld    xD, imm(xA)        # load
st    xS, imm(xA)        # store
mov   xD, imm | xD, xS | LC, imm | EC, imm | pN, bool
loop      target         # backward branch (non-pipelined)
loop.pip  target         # backward branch (software-pipelined)
```

---

## How it works

The scheduler is a small compiler pipeline:

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

1. **Parse** (`parser.rs`) — text assembly into a typed `Instruction` enum.
2. **Dependency analysis** (`dependency.rs`) — for every operand, find the
   producing instruction and classify the edge. Loops make this non-trivial, so
   dependencies are split into four kinds:
   - **Local** — producer and consumer in the same straight-line region.
   - **Inter-loop** — a value produced in one iteration and read in the next.
   - **Loop-invariant** — produced before the loop, read inside it.
   - **Post-loop** — produced inside the loop, read after it exits.
3. **Scheduling** — assign each instruction to a bundle (cycle) and a functional
   unit so that no latency or structural constraint is violated.
   - Non-pipelined (`scheduler.rs`): ASAP list scheduling.
   - Pipelined (`pip_scheduler.rs`): **modulo scheduling** — search for the
     smallest *Initiation Interval* (II) at which a new iteration can start, and
     fold the loop body into that II-cycle steady state.
4. **Register allocation** (`scheduler.rs`, `pip_reg_alloc.rs`) — rename
   destinations to remove false dependencies. The pipelined path additionally
   assigns **rotating registers** across overlapping iterations and inserts
   **predicates** so the prologue and epilogue fill and drain the pipeline
   correctly.

### Module map

| File                   | Responsibility                                        |
|------------------------|-------------------------------------------------------|
| `src/vliw470.rs`       | Entry point: I/O and pipeline wiring                  |
| `src/instruction.rs`   | Instruction / bundle / functional-unit model          |
| `src/parser.rs`        | Assembly → `Instruction`                              |
| `src/dependency.rs`    | Dependency graph construction & classification         |
| `src/scheduler.rs`     | Non-pipelined scheduling + register renaming           |
| `src/pip_scheduler.rs` | Modulo scheduler (finds II, lays out the loop)         |
| `src/pip_reg_alloc.rs` | Rotating-register allocation & predication             |

---

## Build & run

```bash
./build.sh          # cargo build --release

# Schedule one program two ways:
#   ./run.sh <input> <loop_out> <pip_out>
./run.sh tests/17/input.json out.simple.json out.pip.json
```

Or call the binary directly:

```bash
cargo run --release -- <input.json> <output.json> --no_pip   # loop schedule
cargo run --release -- <input.json> <output.json> --pip      # software-pipelined
```

### Input / output format

Input is a JSON array of assembly lines; output is a JSON array of bundles, each
a 5-element array `[ALU0, ALU1, Mult, Mem, Branch]`.

```jsonc
// in: a loop that loads, accumulates with a loop-invariant, and stores
["mov LC, 100", "mov x2, 10", "mov x3, 0x1000",
 "ld x4, 0(x3)", "add x4, x4, x2", "st x4, 0(x3)", "loop 3"]
```

```jsonc
// out (--no_pip): registers renamed, load hoisted to hide its latency
[[" mov LC, 100",  " mov x1, 10", " nop", " nop",          " nop"],
 [" mov x2, 4096", " nop",        " nop", " nop",          " nop"],
 [" nop",          " nop",        " nop", " ld x3, 0(x2)", " nop"],
 [" add x4, x3, x1"," nop",       " nop", " nop",          " nop"],
 [" nop",          " nop",        " nop", " st x4, 0(x2)", " loop 2"]]
```

In the **`--pip`** output, loop-body operations carry rotating registers and
predicates (`(p32) ...`) so successive iterations overlap in the steady state —
that is the software pipeline.

---

## Testing

`tests/` contains 17 cases, each a folder with an `input.json`, a one-line
`desc.txt`, and reference outputs (`simple_ref.json`, `pip_ref.json`). They
cover the full feature ladder: slot mapping, single- and multi-cycle data
hazards, dependency chains, and every loop dependency class.

```bash
./runall.sh     # generate simple.json + pip.json for every test
./testall.sh    # diff each against its reference, print pass/fail
```

`compare.py` does a structural comparison that is tolerant of the two valid
ALU-slot orderings. The Rust unit tests cover the analysis and scheduling
internals directly:

```bash
cargo test
```

Current status: **17/17 reference cases pass** for both schedule modes, and all
unit tests pass.

A small simulator and HTML visualizer live in `simulator/` for inspecting a
schedule cycle by cycle — see `simulator/Readme.md`.

---

## Reproducible environment

A `Dockerfile` pins a Rust + Python toolchain so the build and tests run
identically anywhere:

```bash
docker build -t vliw470 .
docker run -it -v "$(pwd)":/work vliw470
# then, inside:  ./runall.sh && ./testall.sh
```

---

*Originally built as a two-person systems project exploring VLIW compiler
back-ends; cleaned up and documented here.*
