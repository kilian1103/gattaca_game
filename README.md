## Gattaca Challenge

Ant colony destruction simulation implemented in Rust with parallel ant movement and efficient string interning. The program loads a world map, spawns N ants across colonies, iteratively moves them, and destroys colonies when ants collide.

### Quick Start

- **Prerequisites**: Rust (stable) and Cargo installed.
- **Build**:

```bash
cargo build
```

- **Run (N ants)**: pass the number of ants as the first CLI argument.

```bash
cargo run 1000
```

Notes:

- If you omit N, the program exits with: "Please provide a valid ants size".
- The default map is `data/hiveum_map_small.txt`. To switch maps, edit `data_file_path` in `src/main.rs` (line 13).
### Main Optimizations from initial version

- Use multithreading for evolving ants with world map (read-op)
- Use singlethread for evolving world map (write-op)
- Use early return if no collisions
- Use HashMap or HashSets if possible for quick read, write and checks
- Use HashSet instead of Vec of unique items for faster access of items
- Use chunking rather than iter for each thread to process ant movements
- Remove String clone operations and replace wth String interning
- Compress for-loops that create new Datastructures with iterators or in-built methods for in-place modification

### Benchmarks
- for N=10.000 iterations or early termination
- Benchmark = difference of after world_map loading and before final map printing
- Local machine:
  - CPU’s: 12
  - Threads: 12

- Medium,
  - N=6000, 1600 miliseconds, (250 miliiseconds in --release mode)
  - N=600, 200 milliseconds, (30 milliseconds in --release mode)
  - N=60, 200 miliseconds, (70 milliseconds in --release mode)

- Small:
  - N=200, 1 milliseonds, (1 milliseconds in --release mode)
  - N=20, 20 milliseconds, (2 milliseconds in --release mode)
  - N=2, 1 mlliiescond, (1 milliseconds in --release mode)



### Data Format

Each line defines a colony and its exits as direction=destination pairs:

```text
Colony north=Other south=Another east=Foo west=Bar
```

Valid directions are printed in the order: north, south, east, west. See `data/hiveum_map_small.txt` and `data/hiveum_map_medium.txt` for examples.

### How It Works

High-level iteration (up to 10,000 steps or until all ants die):

1. Parallel ant movement: each ant moves uniformly at random to one available neighboring colony (if any).
2. Collision detection (single-threaded): any colony with 2+ ants explodes; all those ants die.
3. World update: remove destroyed colonies and delete inbound tunnels from neighbors.
4. Termination: when all ants are dead or after the loop completes, remaining colonies are printed.

### Core Data Structures

- **World map**: `HashMap<Spur, HashMap<Spur, Spur>>`
  - Keys: `Spur` (interned string) colony identifiers.
  - Values: adjacency map from interned direction → interned destination colony.
- **String interning**: `lasso::Rodeo` (reduces memory and speeds comparisons for colony/direction strings).
- **Ant positions**: `Vec<(usize, Spur)>` where each tuple is `(ant_id, colony_key)`.
- **Collision accounting**: `HashMap<Spur, Vec<usize>>` to collect ant IDs per colony; `HashSet<usize>` for dead ants; `HashSet<Spur>` for doomed colonies; `Vec<(Spur, Spur)>` for neighbor tunnel deletions.
- **Concurrency primitives**: `rayon` for parallel movement (`par_chunks_mut`), `Arc<RwLock<...>>` to share the world and interner safely across threads.

### Concurrency & Performance

- Threads: configured to `num_cpus::get()` using `rayon::ThreadPoolBuilder`.
- Movement phase parallelism: ants are processed in chunks using Rayon.
- Shared state: world map and interner wrapped in `Arc<RwLock<...>>` to enable multi-threaded reads and controlled writes.

### CLI & Configuration

- **Number of ants (N)**: first positional CLI argument.
  - Example: `cargo run 100`
- **Map path**: currently hard-coded in `src/main.rs` (`data_file_path`). Change it to switch maps (e.g., to `data/hiveum_map_medium.txt`).

### Output

- Prints CPU/thread info, iterations, and when colonies are destroyed.
- At the end, prints the remaining colonies and their exits, e.g.:

```text
Colony north=Foo south=Bar
```

- Reports total runtime in milliseconds.

### Dependencies

Key crates used:

- `rayon`: parallel iteration and custom thread pool.
- `rand`: random movement choices.
- `lasso`: string interning (`Rodeo`, `Spur`).
- `num_cpus`: CPU count for thread pool sizing.

### Troubleshooting

- Missing N argument: provide an integer, e.g., `cargo run  100`.
- Empty or malformed map lines are skipped; ensure `data/*.txt` follows the expected "direction=destination" format.
