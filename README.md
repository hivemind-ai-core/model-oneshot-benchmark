
This repository contains a one-shot benchmark comparing:
1. `GLM 5` (running via Claude Code cli)
2. `MiniMax M2.5` (running via Claude Code cli)
3. `Kimi K2.5` (running via Kimi Code cli)

Each were given an empty directory containing the [SPEC.md](./SPEC.md) file. The specification was written by Claude Opus 4.6.
Then each one was prompted with the [PROMPT](./PROMPT.txt).

# Manual testing

```
cargo run -- add "Test task"
cargo run -- add "Task one"
cargo run -- depend 1 2
cargo run -- show 1
cargo run -- target 1
cargo run -- list # Should list "Task one" before "Test task"
cargo run -- current # Should show "Task one"/task #2
cargo run -- edit 2 --dod "Done"
cargo run -- done
cargo run -- next # Should show "Test task"/task #1
```

# Results

### Kimi

* Produced a sparse PLAN.md file: ❌
* Completed first: ✅
* No Rust warnings: ✅
* Passed manual testing without issue: ✅✅
* Did NOT implement MCP server (stub only): ❌

Score: 4/5

### MiniMax

* Produced a detailed PLAN.md file: ✅
* Completed last: ❌
* Many Rust warnings: ❌❌
* Passed manual testing without issue: ✅✅
* Did NOT implement MCP server (stub only): ❌

Score: 3/5

### GLM

* Produced a detailed PLAN.md file: ✅
* Completed second: ✅
* Some Rust warnings: ❌
* Contained a bug! Did NOT complete manual testing: ❌
* Did NOT implement MCP server (stub only): ❌

Score 2/5

## Conclusion

Given the severity of containing a bug, GLM loses an extra point.
For one-shot development: Kimi > MiniMax > GLM

However, it appears that:
1. GLM is better at detailed specs, architecture, and plans
2. MiniMax and Kimi are good at implementation
3. Kimi is best at following multi-step plans
4. GLM is not good at following multi-step plans

Although those obsevrations are not very scientific. YMMV.

