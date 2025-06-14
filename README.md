# what is it?
A tool to keep local repos up to date and clone new repos from a given team.

# benchmarks
## single threaded
cargo run  3.21s user 4.07s system 6% cpu 1:51.83 total

## tokio spawn
cargo run  1.78s user 1.88s system 24% cpu 14.688 total
cargo run  1.71s user 1.73s system 30% cpu 11.267 total

## tokio spawn_blocking
cargo run  1.62s user 2.17s system 71% cpu 5.344 total
